use nalgebra::Unit;

use core::backends::wgpu_backend::WGPUResource;

use crate::taskpool::{TaskPool, TaskPoolBuilder};
use crate::util::{any_as_u8_slice_array, any_as_x_slice_array};
use core::context::{RContext, ResourceRef};
use core::event::{Event, EventSender};
use core::geometry::{Geometry, MeshBuilder, MeshCoordType};
use core::material::basic::BasicMaterialFaceBuilder;
use core::material::{Material, MaterialBuilder};
use core::render::default_blender;
use core::scene::{Camera, RenderObject, Scene, TagId, Transform, TransformBuilder};
use core::types::{BoundBox, Size, Vec2f, Vec3f, Vec4f};
use core::{
    event::{CustomEvent, EventProcessor, EventSource, ProcessEventResult},
    geometry::StaticGeometry,
};
use std::collections::HashMap;
use std::io::{Seek, SeekFrom};
use std::path::PathBuf;
use std::{
    fs::File,
    io::{BufReader, Read},
    sync::{mpsc, Arc, Mutex},
};

enum GltfBuffer<'a> {
    Cursor(std::io::Cursor<&'a Vec<u8>>),
    File(BufReader<File>),
}

impl<'a> GltfBuffer<'a> {
    fn read_bytes(&mut self, accessor: &gltf::Accessor) -> Vec<u8> {
        let offset = accessor.offset() as u64 + accessor.view().unwrap().offset() as u64;
        let size = accessor.size() * accessor.count();
        match self {
            GltfBuffer::Cursor(c) => {
                let _ = c.seek(SeekFrom::Start(offset));
                let mut buf = Vec::new();
                buf.resize(size, 0);
                c.read_exact(&mut buf).unwrap();
                buf
            }
            GltfBuffer::File(f) => {
                let _ = f.seek(SeekFrom::Start(offset));
                let mut buf = Vec::new();
                buf.resize(size, 0);
                f.read_exact(&mut buf).unwrap();
                buf
            }
        }
    }
}

#[derive(Debug)]
enum MaterialFaceBuilder {
    Basic(BasicMaterialFaceBuilder),
}

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
enum MaterialInputKind {
    None,
    Color,
    Texture,
    ColorTexture,
}

impl MaterialInputKind {
    pub fn add_color(self) -> Option<Self> {
        Some(match self {
            MaterialInputKind::None => MaterialInputKind::Color,
            MaterialInputKind::Texture => MaterialInputKind::ColorTexture,
            _ => {
                return None;
            }
        })
    }
    pub fn add_texture(self) -> Option<Self> {
        Some(match self {
            MaterialInputKind::None => MaterialInputKind::Texture,
            MaterialInputKind::Color => MaterialInputKind::ColorTexture,
            _ => {
                return None;
            }
        })
    }
}

#[derive(Debug)]
struct MaterialMap<'a> {
    pub map: HashMap<(Option<usize>, MaterialInputKind), Arc<Material>>,
    pub part_map: Option<HashMap<Option<usize>, (MaterialFaceBuilder, MaterialBuilder)>>,
    context: Option<&'a RContext>,
}

impl<'a> MaterialMap<'a> {
    pub fn new(c: &'a RContext) -> Self {
        Self {
            map: HashMap::new(),
            part_map: Some(HashMap::new()),
            context: Some(c),
        }
    }

    pub fn prepare_kind(&mut self, idx: Option<usize>, kind: MaterialInputKind) -> Arc<Material> {
        let part_map = self.part_map.take().unwrap();
        let context = self.context.take().unwrap();

        let k = self.map.entry((idx, kind)).or_insert_with(|| {
            let (b, m) = part_map.get(&idx).as_ref().unwrap();
            let b = match b {
                MaterialFaceBuilder::Basic(b) => match kind {
                    MaterialInputKind::None => b.clone(),
                    MaterialInputKind::Color => b.clone().with_color(),
                    MaterialInputKind::Texture => b.clone().with_texture(),
                    MaterialInputKind::ColorTexture => b.clone().with_texture().with_color(),
                },
            };
            m.clone().with_face(b.build()).build(context)
        });
        self.part_map = Some(part_map);
        self.context = Some(context);
        k.clone()
    }
}

fn parse_mesh(
    gscene: &mut Scene,
    tag_id: TagId,
    buf_readers: &mut Vec<GltfBuffer>,
    mesh: gltf::Mesh,
    material_map: &mut MaterialMap,
    transform: &Transform,
) -> anyhow::Result<BoundBox> {
    let mut bound_box = BoundBox::default();

    for p in mesh.primitives() {
        let bb = p.bounding_box();
        let mut bb = BoundBox::new(bb.min.into(), bb.max.into());
        bb.mul_mut(transform.mat());

        bound_box = &bound_box + &bb;

        let mut gmesh = MeshBuilder::new();
        let mut color = Vec4f::new(1.0f32, 1.0f32, 1.0f32, 1.0f32);
        let indices = p.indices().unwrap();
        match indices.dimensions() {
            gltf::accessor::Dimensions::Scalar => {}
            _ => {
                anyhow::bail!("dimension for indices invalid");
            }
        }

        let mut kind = MaterialInputKind::None;

        let mut no_position = true;
        for (semantic, _) in p.attributes() {
            match semantic {
                gltf::Semantic::Extras(ext) => {}
                gltf::Semantic::Positions => {
                    no_position = false;
                }
                gltf::Semantic::Normals => {
                    // gmesh.add_props(MeshCoordType::TexNormal);
                }
                gltf::Semantic::Tangents => {}
                gltf::Semantic::Colors(_) => {
                    gmesh.add_props(MeshCoordType::Color);
                }
                gltf::Semantic::TexCoords(_) => {
                    gmesh.add_props(MeshCoordType::TexCoord);
                }
                gltf::Semantic::Joints(_) => {}
                gltf::Semantic::Weights(_) => {}
            }
        }
        if no_position {
            gmesh.set_no_position();
        }
        let mut gmesh = gmesh.finish_props();

        match indices.data_type() {
            gltf::accessor::DataType::U8 => {
                let buf = buf_readers[0].read_bytes(&indices);
                let mut input = Vec::new();
                for d in any_as_x_slice_array::<u8, _>(&buf) {
                    input.push(*d as u32);
                }
                drop(buf);
                gmesh.add_indices(&input);
            }
            gltf::accessor::DataType::U16 => {
                let buf = buf_readers[0].read_bytes(&indices);
                let mut input = Vec::new();
                for d in any_as_x_slice_array::<u16, _>(&buf) {
                    input.push(*d as u32);
                }
                drop(buf);
                gmesh.add_indices(&input);
            }
            gltf::accessor::DataType::U32 => {
                let buf = buf_readers[0].read_bytes(&indices);
                gmesh.add_indices(any_as_x_slice_array(&buf));
            }
            t => {
                anyhow::bail!("data type {:?} for indices is not supported", t)
            }
        }

        for (semantic, accessor) in p.attributes() {
            match semantic {
                gltf::Semantic::Extras(ext) => {
                    log::info!("extra {}", ext);
                }
                gltf::Semantic::Positions => {
                    let buf = buf_readers[0].read_bytes(&accessor);
                    match accessor.data_type() {
                        gltf::accessor::DataType::F32 => {}
                        _ => {
                            anyhow::bail!("position invalid data type");
                        }
                    };
                    match accessor.dimensions() {
                        gltf::accessor::Dimensions::Vec3 => {}
                        _ => {
                            anyhow::bail!("position should be vec3f");
                        }
                    };
                    let f = any_as_x_slice_array(&buf);
                    let mut data = Vec::new();
                    for block in f.chunks(3) {
                        data.push(Vec3f::new(block[0], block[1], block[2]));
                    }

                    gmesh.add_vertices_position(&data);
                }
                gltf::Semantic::Normals => {}
                gltf::Semantic::Tangents => {}
                gltf::Semantic::Colors(_index) => {
                    kind = kind.add_color().unwrap();

                    let buf = buf_readers[0].read_bytes(&accessor);
                    match accessor.data_type() {
                        gltf::accessor::DataType::F32 => {}
                        _ => {
                            anyhow::bail!("color invalid data type");
                        }
                    };
                    match accessor.dimensions() {
                        gltf::accessor::Dimensions::Vec4 => {}
                        _ => {
                            anyhow::bail!("color should be vec3f");
                        }
                    };
                    let f = any_as_x_slice_array(&buf);
                    let mut data = Vec::new();
                    for block in f.chunks(4) {
                        data.push(Vec4f::new(block[0], block[1], block[2], block[3]));
                    }

                    gmesh.add_vertices_prop(
                        MeshCoordType::Color,
                        any_as_u8_slice_array(&data),
                        4 * 4,
                    );
                }
                gltf::Semantic::TexCoords(_index) => {
                    if let Some(v) = kind.add_texture() {
                        kind = v;
                    }

                    let buf = buf_readers[0].read_bytes(&accessor);
                    match accessor.data_type() {
                        gltf::accessor::DataType::F32 => {}
                        _ => {
                            anyhow::bail!("texcoord invalid data type");
                        }
                    };
                    match accessor.dimensions() {
                        gltf::accessor::Dimensions::Vec2 => {}
                        _ => {
                            anyhow::bail!("texcoord should be vec2f");
                        }
                    };

                    let f = any_as_x_slice_array(&buf);
                    let mut data = Vec::new();
                    for block in f.chunks(2) {
                        data.push(Vec2f::new(block[0], block[1]));
                    }

                    gmesh.add_vertices_prop(
                        MeshCoordType::TexCoord,
                        any_as_u8_slice_array(&data),
                        4 * 2,
                    );
                }
                gltf::Semantic::Joints(_index) => {}
                gltf::Semantic::Weights(_index) => {}
            }
        }
        color = p
            .material()
            .pbr_metallic_roughness()
            .base_color_factor()
            .into();

        let idx = p.material().index();

        let material = material_map.prepare_kind(idx, kind);
        let mut g = StaticGeometry::new(Arc::new(gmesh.build()));
        g.set_attribute(core::geometry::Attribute::ConstantColor, Arc::new(color));
        g = g.with_transform(transform.clone());
        let mut obj = RenderObject::new(Box::new(g), material);
        obj.set_name(mesh.name().unwrap_or_default());

        obj.add_tag(tag_id);

        gscene.add(obj);
    }
    log::info!("mesh {:?} {:?}", mesh.name(), transform);

    Ok(bound_box)
}

fn parse_texture(
    texture_map: &mut Mutex<HashMap<usize, ResourceRef>>,
    path: &PathBuf,
    gpu: &WGPUResource,
    texture: gltf::Texture,
) -> anyhow::Result<()> {
    let source = texture.source().source();
    let (data, _ty) = match source {
        gltf::image::Source::View { view, mime_type: _ } => {
            let _buf = view.buffer();
            let _offset = view.offset();
            todo!();
        }
        gltf::image::Source::Uri { uri, mime_type } => {
            log::info!("read uri texture {} with mime {:?}", uri, mime_type);
            let uri = urlencoding::decode(uri)?;
            let target = path.parent().unwrap().join(uri.to_string());

            let mut file = File::open(target)?;
            let mut buf = Vec::new();

            file.read_to_end(&mut buf)?;

            (buf, mime_type)
        }
    };
    let image = image::load_from_memory(&data)?;
    let width = image.width();
    let height = image.height();

    let rgba = image.into_rgba8();
    let rgba = rgba.as_raw();
    let real_texture = gpu.from_rgba_texture(rgba, Size::new(width, height));

    texture_map
        .lock()
        .unwrap()
        .insert(texture.index(), real_texture);
    Ok(())
}

fn parse_node(
    gscene: &mut Scene,
    node: gltf::Node,
    tag_id: TagId,
    buf_readers: &mut Vec<GltfBuffer>,
    material_map: &mut MaterialMap,
    transform: &Transform,
    bound_box: &mut BoundBox,
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
        let bb = parse_mesh(
            gscene,
            tag_id,
            buf_readers,
            mesh,
            material_map,
            &transform_node,
        )?;
        *bound_box = &*bound_box + &bb;
    }

    for node in node.children() {
        parse_node(
            gscene,
            node,
            tag_id,
            buf_readers,
            material_map,
            &transform_node,
            bound_box,
        )?;
    }
    Ok(())
}

fn load(name: &str, pool: &TaskPool, rm: Arc<ResourceManager>) -> anyhow::Result<Scene> {
    let path = PathBuf::from(name);
    let gltf = gltf::Gltf::open(&path)?;
    let mut buf_readers: Vec<GltfBuffer> = Vec::new();
    for buf in gltf.buffers() {
        buf_readers.push(match buf.source() {
            gltf::buffer::Source::Bin => {
                GltfBuffer::Cursor(std::io::Cursor::new(gltf.blob.as_ref().unwrap()))
            }
            gltf::buffer::Source::Uri(uri) => {
                let uri = urlencoding::decode(uri)?;
                let target = path.parent().unwrap().join(uri.to_string());
                log::info!("read gltf uri {}", uri);
                GltfBuffer::File(BufReader::new(File::open(target).unwrap()))
            }
        });
    }
    let mut gscene = rm.new_scene();
    let mut map = MaterialMap::new(rm.gpu.context());
    let mut texture_map = Mutex::new(HashMap::new());

    let batch = pool.make_batch();

    for texture in gltf.textures() {
        batch.execute(|| parse_texture(&mut texture_map, &path, &rm.gpu, texture));
    }
    batch.wait()?;

    let texture_map = texture_map.into_inner()?;

    // add default material
    {
        let primitive = wgpu::PrimitiveState::default();
        let mut material_builder = MaterialBuilder::default();
        material_builder = material_builder.with_primitive(primitive);

        let mut basic_material_builder = BasicMaterialFaceBuilder::default();

        let color = Vec4f::new(1f32, 1f32, 1f32, 1f32);
        basic_material_builder = basic_material_builder.with_constant_color(color);

        map.part_map.as_mut().unwrap().insert(
            None,
            (
                MaterialFaceBuilder::Basic(basic_material_builder),
                material_builder,
            ),
        );
    }

    for material in gltf.materials() {
        let idx = material.index().unwrap_or_default();
        let mut primitive = wgpu::PrimitiveState::default();
        let mut material_builder = MaterialBuilder::default();
        if material.double_sided() {
            primitive.cull_mode = None;
        }
        material_builder = material_builder.with_primitive(primitive);

        let mut basic_material_builder = BasicMaterialFaceBuilder::default();

        let color = material.pbr_metallic_roughness().base_color_factor();
        let texture = material.pbr_metallic_roughness().base_color_texture();
        if let Some(tex) = texture {
            let texture_index = tex.texture().index();
            let texture = texture_map.get(&texture_index).unwrap();
            basic_material_builder = basic_material_builder.with_texture_data(texture.clone());
        }

        match material.alpha_mode() {
            gltf::material::AlphaMode::Opaque => {}
            gltf::material::AlphaMode::Mask => {
                basic_material_builder.enable_alpha_test();
                material_builder =
                    material_builder.with_alpha_test(material.alpha_cutoff().unwrap_or(0.5f32));
            }
            gltf::material::AlphaMode::Blend => {
                material_builder = material_builder.with_blend(default_blender());
            }
        }

        basic_material_builder = basic_material_builder.with_constant_color(color.into());

        map.part_map.as_mut().unwrap().insert(
            Some(idx),
            (
                MaterialFaceBuilder::Basic(basic_material_builder),
                material_builder,
            ),
        );
    }
    let mut bound_box = BoundBox::default();

    for s in gltf.scenes() {
        let name = s.name().unwrap_or("gltf-scene");
        let tag_id = gscene.new_tag(name);

        let transform = Transform::default();
        for node in s.nodes() {
            parse_node(
                &mut gscene,
                node,
                tag_id,
                &mut buf_readers,
                &mut map,
                &transform,
                &mut bound_box,
            )?;
        }
        log::info!(
            "model scene {} nodes {}",
            s.name().unwrap_or_default(),
            s.nodes().len()
        );
    }
    let aspect = rm.gpu.aspect();
    let ctx = rm.gpu.context();

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
                    let camera = Camera::new(ctx);
                    camera.make_orthographic(
                        Vec4f::new(c.xmag(), c.ymag(), c.xmag(), c.ymag()),
                        c.znear(),
                        c.zfar(),
                    );
                    camera
                }
                gltf::camera::Projection::Perspective(c) => {
                    let camera = Camera::new(ctx);
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

            let camera = Camera::new(ctx);
            camera.make_perspective(aspect, std::f32::consts::FRAC_PI_3, near, far);
            camera
        }
    };

    camera.look_at(from, center, Vec3f::new(0f32, 1f32, 0f32));
    let camera = Arc::new(camera);
    log::info!(
        "bound box {:?} with camera {:?} distance {}",
        bound_box,
        camera,
        dist
    );

    gscene.set_main_camera(camera);

    Ok(gscene)
}

fn loader_main(rx: mpsc::Receiver<(String, Box<dyn EventSender>)>, rm: Arc<ResourceManager>) {
    let pool = TaskPoolBuilder::new().build();
    loop {
        let (name, proxy) = rx.recv().unwrap();
        if name.is_empty() {
            break;
        }
        let result = load(&name, &pool, rm.clone());
        let result = match result {
            Ok(val) => val,
            Err(err) => {
                log::error!("{} in {}", err, name);
                continue;
            }
        };

        log::info!("load model {}", name);
        proxy.send_event(Event::CustomEvent(CustomEvent::Loaded(rm.insert(result))));
    }
}

pub struct Loader {
    thread: Option<std::thread::JoinHandle<()>>,
    tx: mpsc::Sender<(String, Box<dyn EventSender>)>,
    resource_manager: Arc<ResourceManager>,
}

impl Loader {
    pub fn new(resource_manager: Arc<ResourceManager>) -> Self {
        let (tx, rx) = mpsc::channel();
        let rm = resource_manager.clone();
        let mut this = Self {
            thread: None,
            tx,
            resource_manager,
        };
        this.thread = Some(std::thread::spawn(move || {
            loader_main(rx, rm);
        }));
        this
    }
    pub fn event_processor(&self) -> Box<LoaderEventProcessor> {
        Box::new(LoaderEventProcessor {
            tx: self.tx.clone(),
        })
    }
}

pub struct LoaderEventProcessor {
    tx: mpsc::Sender<(String, Box<dyn EventSender>)>,
}

impl EventProcessor for LoaderEventProcessor {
    fn on_event(&mut self, source: &dyn EventSource, event: &Event) -> ProcessEventResult {
        match event {
            Event::CustomEvent(e) => match e {
                CustomEvent::Loading(name) => {
                    let _ = self.tx.send((name.clone(), source.new_event_sender()));
                    ProcessEventResult::Consumed
                }
                _ => ProcessEventResult::Received,
            },
            _ => ProcessEventResult::Received,
        }
    }
}

#[derive(Debug, Default)]
struct ResourceManagerInner {
    scene_map: HashMap<u64, Scene>,
    last_id: u64,
}

#[derive(Debug)]
pub struct ResourceManager {
    inner: Mutex<ResourceManagerInner>,
    gpu: Arc<WGPUResource>,
}

impl ResourceManager {
    pub fn new(gpu: Arc<WGPUResource>) -> Self {
        Self {
            inner: Mutex::new(ResourceManagerInner::default()),
            gpu,
        }
    }
}

impl ResourceManager {
    pub fn insert(&self, scene: Scene) -> u64 {
        let mut inner = self.inner.lock().unwrap();
        let id = inner.last_id;
        inner.last_id += 1;
        inner.scene_map.insert(id, scene);

        id
    }

    pub fn new_scene(&self) -> Scene {
        Scene::new(self.gpu.context_ref())
    }

    pub fn take(&self, id: u64) -> Scene {
        let mut inner = self.inner.lock().unwrap();
        inner.scene_map.remove(&id).unwrap()
    }
}
