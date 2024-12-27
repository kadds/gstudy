use app::plugin::{Plugin, PluginFactory};
use app::AppEventProcessor;
use gltf::texture::Sampler;
use material_loader::basic_loader::BasicMaterialLoader;
use material_loader::MaterialLoader;
use nalgebra::Unit;
mod taskpool;

use core::backends::wgpu_backend::WGPUResource;
use core::mesh::builder::{MeshBuilder, MeshPropertiesBuilder};
use core::wgpu;
use std::any::Any;

use core::context::{RContext, RContextRef, ResourceRef, TagId};
use core::mesh::StaticGeometry;
use core::scene::{Camera, RenderObject, Scene, Transform, TransformBuilder};
use core::types::{BoundBox, Size, Vec3f, Vec4f};
use core::util::any_as_x_slice_array;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::{
    fs::File,
    sync::{mpsc, Arc, Mutex},
};
use taskpool::{TaskPool, TaskPoolBuilder};

pub mod material_loader;

#[derive(Debug, Default)]
struct SourcePosition {
    size: usize,
    offset: u64,
}

enum GltfDataViewSource<'a> {
    Cursor((SourcePosition, &'a Vec<u8>)),
    File((SourcePosition, memmap2::Mmap, File)),
}

impl<'a> GltfDataViewSource<'a> {
    fn new_file(mut source_position: SourcePosition, file: &Path) -> anyhow::Result<Self> {
        let file = File::open(file)?;
        if source_position.size == 0 {
            source_position.size = file.metadata().unwrap().len() as usize;
        }

        let m = unsafe { memmap2::Mmap::map(&file)? };
        #[cfg(unix)]
        {
            let _ = m.advise(memmap2::Advice::Sequential);
        }
        Ok(Self::File((source_position, m, file)))
    }

    fn new_cursor(source_position: SourcePosition, buf: &'a Vec<u8>) -> anyhow::Result<Self> {
        Ok(Self::Cursor((source_position, buf)))
    }

    fn read_bytes_from_accessor(&'a self, accessor: &gltf::Accessor) -> &'a [u8] {
        let offset = accessor.offset() as u64 + accessor.view().unwrap().offset() as u64;
        let size = accessor.size() * accessor.count();
        let end = offset as usize + size;
        match self {
            GltfDataViewSource::Cursor((_, c)) => &c[offset as usize..end],
            GltfDataViewSource::File((_, m, _)) => &m[offset as usize..end],
        }
    }

    fn read_bytes(&'a self) -> &'a [u8] {
        match self {
            GltfDataViewSource::Cursor((s, c)) => {
                let end = s.offset as usize + s.size;
                &c[s.offset as usize..end]
            }
            GltfDataViewSource::File((s, m, _)) => {
                let end = s.offset as usize + s.size;
                &m[s.offset as usize..end]
            }
        }
    }
}

pub type TextureMap = HashMap<usize, (Option<usize>, ResourceRef)>;

fn parse_texture(
    texture_map: &mut Mutex<TextureMap>,
    buf_view: &GltfBufferView,
    gpu: &WGPUResource,
    texture: gltf::Texture,
) -> anyhow::Result<()> {
    let source_index = texture.source().index();
    let r = &buf_view.texture[source_index];

    let data = r.read_bytes();

    let image = image::load_from_memory(data)?;
    let width = image.width();
    let height = image.height();

    let rgba = image.into_rgba8();
    let rgba = rgba.as_raw();
    let real_texture = gpu.from_rgba_texture(rgba, Size::new(width, height));

    texture_map
        .lock()
        .unwrap()
        .insert(texture.index(), (texture.sampler().index(), real_texture));
    Ok(())
}

fn parse_sampler(original: &Sampler, gpu: &WGPUResource) -> ResourceRef {
    let mut desc = wgpu::SamplerDescriptor::default();
    if let Some(filter) = original.mag_filter() {
        desc.mag_filter = match filter {
            gltf::texture::MagFilter::Nearest => wgpu::FilterMode::Nearest,
            gltf::texture::MagFilter::Linear => wgpu::FilterMode::Linear,
        }
    }
    if let Some(filter) = original.min_filter() {
        desc.min_filter = match filter {
            gltf::texture::MinFilter::Nearest => wgpu::FilterMode::Nearest,
            gltf::texture::MinFilter::Linear => wgpu::FilterMode::Linear,
            gltf::texture::MinFilter::NearestMipmapNearest => wgpu::FilterMode::Nearest,
            gltf::texture::MinFilter::LinearMipmapNearest => wgpu::FilterMode::Nearest,
            gltf::texture::MinFilter::NearestMipmapLinear => wgpu::FilterMode::Linear,
            gltf::texture::MinFilter::LinearMipmapLinear => wgpu::FilterMode::Linear,
        };
        match filter {
            gltf::texture::MinFilter::NearestMipmapNearest => {
                desc.mipmap_filter = desc.min_filter;
            }
            gltf::texture::MinFilter::LinearMipmapNearest => {
                desc.mipmap_filter = desc.min_filter;
            }
            gltf::texture::MinFilter::NearestMipmapLinear => {
                desc.mipmap_filter = desc.min_filter;
            }
            gltf::texture::MinFilter::LinearMipmapLinear => {
                desc.mipmap_filter = desc.min_filter;
            }
            _ => {}
        }
    }

    desc.address_mode_u = match original.wrap_s() {
        gltf::texture::WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        gltf::texture::WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
        gltf::texture::WrappingMode::Repeat => wgpu::AddressMode::Repeat,
    };

    desc.address_mode_v = match original.wrap_t() {
        gltf::texture::WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        gltf::texture::WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
        gltf::texture::WrappingMode::Repeat => wgpu::AddressMode::Repeat,
    };

    gpu.from_sampler(&desc)
}

#[derive(Debug, Default)]
pub struct GltfSceneInfo {
    total_nodes: u64,
    total_meshes: u64,
    total_vertices: u64,
    total_indices: u64,
    total_textures: u64,
    total_samplers: u64,
    aabb: BoundBox,
    main_camera_position: Vec3f,
    main_camera_direction: Vec3f,
}

pub struct GltfBufferView<'a> {
    buffer: Vec<GltfDataViewSource<'a>>,
    texture: Vec<GltfDataViewSource<'a>>,
}

pub struct ParseContext<'a> {
    pool: &'a TaskPool,
    gpu: Arc<WGPUResource>,
    ctx: RContextRef,
    // opened_files: HashMap<Box<File>>,
    scene: Scene,
    info: GltfSceneInfo,
    material_loader: Box<RefCell<dyn MaterialLoader>>,
}

fn parse_primitive_indices(
    p: &gltf::Primitive,
    mesh_builder: &mut MeshBuilder,
    buf_view: &GltfBufferView,
    res: &mut GltfSceneInfo,
) -> anyhow::Result<()> {
    if let Some(indices) = p.indices() {
        match indices.dimensions() {
            gltf::accessor::Dimensions::Scalar => {}
            _ => {
                anyhow::bail!("dimension for indices invalid");
            }
        }

        match indices.data_type() {
            gltf::accessor::DataType::U8 => {
                let buf = buf_view.buffer[0].read_bytes_from_accessor(&indices);
                let mut input = Vec::new();
                for d in any_as_x_slice_array::<u8, _>(buf) {
                    input.push(*d as u32);
                }
                res.total_indices += (buf.len() / std::mem::size_of::<u8>()) as u64;
                mesh_builder.add_indices32(&input);
            }
            gltf::accessor::DataType::U16 => {
                let buf = buf_view.buffer[0].read_bytes_from_accessor(&indices);

                let mut input = Vec::new();
                for d in any_as_x_slice_array::<u16, _>(buf) {
                    input.push(*d as u32);
                }
                res.total_indices += (buf.len() / std::mem::size_of::<u16>()) as u64;
                mesh_builder.add_indices32(&input);
            }
            gltf::accessor::DataType::U32 => {
                let buf = buf_view.buffer[0].read_bytes_from_accessor(&indices);
                mesh_builder.add_indices32(any_as_x_slice_array(buf));
                res.total_indices += (buf.len() / std::mem::size_of::<u32>()) as u64;
            }
            t => {
                anyhow::bail!("data type {:?} for indices is not supported", t)
            }
        };
    } else {
        mesh_builder.add_indices_none();
    }
    Ok(())
}

impl<'a> ParseContext<'a> {
    fn parse_mesh(
        &mut self,
        tag_id: TagId,
        buf_view: &mut GltfBufferView<'a>,
        mesh: gltf::Mesh,
        transform: &Transform,
    ) -> anyhow::Result<BoundBox> {
        self.info.total_meshes += 1;

        let mut bound_box = BoundBox::default();
        for p in mesh.primitives() {
            let bb = p.bounding_box();
            let mut bb = BoundBox::new(bb.min.into(), bb.max.into());
            bb.mul_mut(transform.mat());

            bound_box = &bound_box + &bb;

            let mut mesh_builder = MeshBuilder::default();
            let mut mesh_properties_builder = MeshPropertiesBuilder::default();

            parse_primitive_indices(&p, &mut mesh_builder, buf_view, &mut self.info)?;

            let material = self.material_loader.borrow_mut().load_properties_vertices(
                &p,
                &mut mesh_builder,
                &mut mesh_properties_builder,
                buf_view,
                &mut self.info,
            )?;

            mesh_builder.set_properties(mesh_properties_builder.build());

            let mut g = StaticGeometry::new(Arc::new(mesh_builder.build()?));

            g = g.with_transform(transform.clone());
            let mut obj = RenderObject::new(Box::new(g), material.clone()).unwrap();
            obj.set_cast_shadow();
            obj.set_recv_shadow();
            obj.set_name(mesh.name().unwrap_or_default());

            obj.add_tag(tag_id);

            self.scene.add(obj);
        }
        log::info!("load mesh {:?} transform {:?}", mesh.name(), transform);

        Ok(bound_box)
    }

    fn parse_node(
        &mut self,
        node: gltf::Node,
        tag_id: TagId,
        buf: &mut GltfBufferView<'a>,
        transform: &Transform,
    ) -> anyhow::Result<()> {
        let d = node.transform().decomposed();
        let q = nalgebra::Quaternion::from(Vec4f::new(d.1[0], d.1[1], d.1[2], d.1[3]));
        let q = Unit::new_unchecked(q);

        let mut transform_node = TransformBuilder::new()
            .translate(d.0.into())
            .rotate(q)
            .scale(d.2.into())
            .build();
        transform_node.mul_mut(transform);

        if let Some(mesh) = node.mesh() {
            let bb = self.parse_mesh(tag_id, buf, mesh, &transform_node)?;
            self.info.aabb = &self.info.aabb + &bb;
        }
        self.info.total_nodes += 1;

        for node in node.children() {
            self.parse_node(node, tag_id, buf, &transform_node)?;
        }
        if let Some(light) = node.light() {
            self.material_loader
                .borrow()
                .load_light(&light, &self.scene)?;
        }
        Ok(())
    }
}

// loader
impl<'a> ParseContext<'a> {
    fn load_materials(
        &mut self,
        gltf: &gltf::Gltf,
        texture_map: &TextureMap,
        samplers: &[ResourceRef],
    ) -> anyhow::Result<()> {
        for material in gltf.materials() {
            let idx = material.index().unwrap_or_default();
            self.material_loader.borrow_mut().load_material(
                idx,
                &material,
                texture_map,
                samplers,
            )?;
        }
        Ok(())
    }

    fn load_buffers(
        &mut self,
        gltf: &'a gltf::Gltf,
        path: &Path,
    ) -> anyhow::Result<GltfBufferView<'a>> {
        let blob_size = gltf.blob.as_ref().map(|v| v.len()).unwrap_or(0);

        let mut buf_readers: Vec<GltfDataViewSource> = Vec::new();
        for buf in gltf.buffers() {
            let r = match buf.source() {
                gltf::buffer::Source::Bin => {
                    let blob = gltf.blob.as_ref().ok_or(anyhow::anyhow!("no blob"))?;
                    GltfDataViewSource::new_cursor(
                        SourcePosition {
                            size: blob_size,
                            offset: 0,
                        },
                        blob,
                    )
                }
                gltf::buffer::Source::Uri(uri) => {
                    let uri = urlencoding::decode(uri)?;
                    let target = path.parent().unwrap().join(uri.to_string());
                    log::info!("read uri buffer {}", uri);
                    GltfDataViewSource::new_file(SourcePosition::default(), &target)
                }
            }?;
            buf_readers.push(r);
        }

        let mut image_readers: Vec<GltfDataViewSource> = Vec::new();
        for image in gltf.images() {
            let r = match image.source() {
                gltf::image::Source::View { view, mime_type: _ } => {
                    let buf = view.buffer();
                    let offset = view.offset() as u64;
                    let size = view.length();
                    match buf.source() {
                        gltf::buffer::Source::Bin => {
                            let blob = gltf.blob.as_ref().ok_or(anyhow::anyhow!("no blob"))?;
                            GltfDataViewSource::new_cursor(SourcePosition { size, offset }, blob)
                        }
                        gltf::buffer::Source::Uri(uri) => {
                            let uri = urlencoding::decode(uri)?;
                            let target = path.parent().unwrap().join(uri.to_string());
                            log::info!("read uri texture {}", uri);
                            GltfDataViewSource::new_file(SourcePosition { size, offset }, &target)
                        }
                    }
                }
                gltf::image::Source::Uri { uri, mime_type } => {
                    log::info!("read uri texture {} with mime {:?}", uri, mime_type);
                    let uri = urlencoding::decode(uri)?;
                    let target = path.parent().unwrap().join(uri.to_string());
                    GltfDataViewSource::new_file(SourcePosition::default(), &target)
                }
            }?;

            image_readers.push(r);
        }

        Ok(GltfBufferView {
            buffer: buf_readers,
            texture: image_readers,
        })
    }

    fn load_textures(
        &mut self,
        gltf: &gltf::Gltf,
        buf_view: &GltfBufferView,
    ) -> anyhow::Result<(TextureMap, Vec<ResourceRef>)> {
        let mut texture_map = Mutex::new(TextureMap::new());

        let batch = self.pool.make_batch();

        for texture in gltf.textures() {
            batch.execute(|| parse_texture(&mut texture_map, buf_view, &self.gpu, texture));
            self.info.total_textures += 1;
        }

        let mut samplers = vec![];
        for sampler in gltf.samplers() {
            let sampler = parse_sampler(&sampler, &self.gpu);
            samplers.push(sampler);
            self.info.total_samplers += 1;
        }

        batch.wait()?;

        let texture_map = texture_map.into_inner()?;
        Ok((texture_map, samplers))
    }

    fn load_meshes(
        &mut self,
        gltf: &gltf::Gltf,
        buf_view: &mut GltfBufferView<'a>,
    ) -> anyhow::Result<()> {
        for s in gltf.scenes() {
            let name = s.name().unwrap_or("gltf-scene");
            let tag_id = self.scene.context().new_tag(name);

            let transform = Transform::default();
            for node in s.nodes() {
                self.parse_node(node, tag_id, buf_view, &transform)?;
            }
            log::info!(
                "model scene {} nodes {}",
                s.name().unwrap_or_default(),
                s.nodes().len()
            );
        }

        Ok(())
    }

    fn load_cameras(&mut self, gltf: &gltf::Gltf) -> anyhow::Result<()> {
        let aspect = self.gpu.aspect();

        let bound_box = &mut self.info.aabb;

        let center = bound_box.center();
        let size = bound_box.size();
        let mut from = Vec3f::default();

        from.x = center.x + bound_box.size().x / 1.5f32;
        from.y = center.y + bound_box.size().y / 1.5f32;
        from.z = center.z + bound_box.size().z * 3f32;

        if (from.z - center.z).abs() < 0.0001f32 {
            from.z += size.max() / 2f32;
        }

        let dist = nalgebra::distance(&from.into(), &center.into());

        let mut from_max_point = from;
        // from_max_point.z = bound_box.max().z + size.max() * 4f32;
        // if from_max_point.z > from.z {
        from_max_point.z = from.z - bound_box.size().z / 100f32;
        // }

        let mut from_min_point = from;
        from_min_point.z = bound_box.min().z - size.max() * 100f32;

        let camera = match gltf.cameras().next() {
            Some(c) => {
                log::info!("scene camera {}", c.name().unwrap_or_default());
                match c.projection() {
                    gltf::camera::Projection::Orthographic(c) => {
                        let camera = Camera::new();
                        camera.make_orthographic(
                            Vec4f::new(c.xmag(), c.ymag(), c.xmag(), c.ymag()),
                            c.znear(),
                            c.zfar(),
                        );
                        camera
                    }
                    gltf::camera::Projection::Perspective(c) => {
                        let camera = Camera::new();
                        camera.make_perspective(
                            c.aspect_ratio().unwrap_or(aspect),
                            c.yfov(),
                            c.znear(),
                            c.zfar().unwrap_or(100_000_f32),
                        );
                        camera
                    }
                }
            }
            None => {
                let near = nalgebra::distance(&from.into(), &from_max_point.into());
                let far = nalgebra::distance(&from.into(), &from_min_point.into());

                let camera = Camera::new();
                camera.make_perspective(aspect, std::f32::consts::FRAC_PI_3, near, far);
                camera
            }
        };

        camera.look_at(from, center, Vec3f::new(0f32, 1f32, 0f32));
        self.info.main_camera_position = from;
        self.info.main_camera_direction = from - center;

        let camera = Arc::new(camera);
        log::info!(
            "bound box {:?} with camera {:?} distance {}",
            bound_box,
            camera,
            dist
        );
        self.scene.set_main_camera(camera);
        Ok(())
    }

    fn load(
        path: &Path,
        pool: &'a TaskPool,
        ctx: RContextRef,
        gpu: Arc<WGPUResource>,
        loader: Box<RefCell<dyn MaterialLoader>>,
    ) -> anyhow::Result<(Scene, GltfSceneInfo)> {
        let gltf = gltf::Gltf::open(path)?;
        let mut this = Self {
            scene: Scene::new(ctx.clone()),
            info: GltfSceneInfo::default(),
            gpu,
            ctx,
            pool,
            material_loader: loader,
        };
        let mut buf_view = this.load_buffers(&gltf, path)?;

        let (textures, samplers) = this.load_textures(&gltf, &buf_view)?;
        this.load_materials(&gltf, &textures, &samplers)?;

        this.load_meshes(&gltf, &mut buf_view)?;
        this.load_cameras(&gltf)?;

        this.material_loader
            .borrow_mut()
            .post_load(&this.scene, &this.info)?;

        Ok((this.scene, this.info))
    }
}

fn default_material_loader(
    loader_name: &str,
    gpu: Arc<WGPUResource>,
) -> Box<RefCell<dyn MaterialLoader>> {
    match loader_name {
        "" | "basic" => {
            return Box::new(RefCell::new(BasicMaterialLoader::new(gpu)));
        }
        "phong" => {
            #[cfg(feature = "phong")]
            {
                use material_loader::phong_loader::PhongMaterialLoader;
                return Box::new(RefCell::new(PhongMaterialLoader::new(gpu)));
            }
        }
        _ => {}
    };

    Box::new(RefCell::new(BasicMaterialLoader::new(gpu)))
}

pub struct LoadSceneResult {
    pub name: String,
    pub scene: Option<Arc<Scene>>,
    pub error_string: String,
}

fn loader_main(
    rx: mpsc::Receiver<(String, String, Arc<WGPUResource>)>,
    tx: Arc<Mutex<VecDeque<LoadSceneResult>>>,
    ctx: RContextRef,
) {
    let pool = TaskPoolBuilder::new().build();
    loop {
        let (name, loader, gpu) = match rx.recv() {
            Ok(e) => e,
            Err(e) => {
                log::warn!("{}", e);
                break;
            }
        };
        if name.is_empty() {
            break;
        }

        let path = PathBuf::from(name.clone());
        let loader = default_material_loader(&loader, gpu.clone());

        let result = ParseContext::load(&path, &pool, ctx.clone(), gpu, loader);
        let (scene, result) = match result {
            Ok(val) => val,
            Err(err) => {
                log::error!("{} in {}", err, name);
                tx.lock().unwrap().push_back(LoadSceneResult {
                    name,
                    scene: None,
                    error_string: format!("{}", err),
                });
                continue;
            }
        };

        log::info!("load model {} {:?}", name, result);
        tx.lock().unwrap().push_back(LoadSceneResult {
            name,
            scene: Some(Arc::new(scene)),
            error_string: String::new(),
        });
    }
}

pub struct Loader {
    thread: Option<std::thread::JoinHandle<()>>,
    tx: mpsc::Sender<(String, String, Arc<WGPUResource>)>,
    result: Arc<Mutex<VecDeque<LoadSceneResult>>>,
    gpu: Mutex<Option<Arc<WGPUResource>>>,
}

impl Loader {
    pub fn new(ctx: RContextRef) -> Self {
        let (tx, rx) = mpsc::channel();
        let mut this = Self {
            thread: None,
            tx,
            result: Arc::new(Mutex::new(VecDeque::new())),
            gpu: Mutex::new(None),
        };

        let res = this.result.clone();

        this.thread = Some(std::thread::spawn(move || {
            loader_main(rx, res, ctx.clone());
        }));
        this
    }

    pub fn load_async<S: Into<String>>(&self, name: S, loader_name: &str) {
        let _ = self.tx.send((
            name.into(),
            loader_name.to_owned(),
            self.gpu.lock().unwrap().clone().unwrap(),
        ));
    }

    fn take_result(&self) -> Vec<LoadSceneResult> {
        let mut result = self.result.lock().unwrap();
        if result.is_empty() {
            return vec![];
        }
        let mut tmp = VecDeque::new();
        std::mem::swap(&mut tmp, &mut *result);

        tmp.into_iter().collect()
    }
}

pub struct GltfPluginFactory;

impl PluginFactory for GltfPluginFactory {
    fn create(&self, container: &app::container::Container) -> Box<dyn Plugin> {
        let ctx = container.get::<RContext>().unwrap();
        let loader = Arc::new(Loader::new(ctx));
        container.register_arc(loader.clone());

        Box::new(GltfPlugin { loader })
    }

    fn info(&self) -> app::plugin::PluginInfo {
        app::plugin::PluginInfo {
            name: "gltfloader".into(),
            version: "0.1.0".into(),
            has_looper: false,
        }
    }
}

pub struct GltfPlugin {
    loader: Arc<Loader>,
}

impl Plugin for GltfPlugin {}

impl AppEventProcessor for GltfPlugin {
    fn on_event(&mut self, context: &app::AppEventContext, event: &dyn Any) {
        if let Some(event) = event.downcast_ref::<core::event::Event>() {
            match event {
                // core::event::Event::FirstSync => {
                //     let mut gpu = self.loader.gpu.lock().unwrap();
                //     if gpu.is_none() {
                //         let g = context.container.get::<WGPUResource>().unwrap();
                //         *gpu = Some(g);
                //     }
                // }
                core::event::Event::PostRender => {
                    let res = self.loader.take_result();
                    for r in res {
                        context
                            .source
                            .event_sender()
                            .send_event(Box::new(Event::Loaded(r)))
                    }
                }
                _ => (),
            }
        }
    }
}

pub enum Event {
    Loaded(LoadSceneResult),
    Unknown,
}
