use app::plugin::{Plugin, PluginFactory};
use app::AppEventProcessor;
use gltf::texture::Sampler;
use nalgebra::Unit;
mod taskpool;

use core::backends::wgpu_backend::WGPUResource;
use core::mesh::builder::MeshBuilder;
use std::any::Any;

use core::context::{RContext, RContextRef, ResourceRef, TagId};
use core::material::basic::BasicMaterialFaceBuilder;
use core::material::{Material, MaterialBuilder};
use core::mesh::StaticGeometry;
use core::mesh::{Geometry, MeshPropertyType};
use core::render::default_blender;
use core::scene::{Camera, RenderObject, Scene, Transform, TransformBuilder};
use core::types::{BoundBox, Color, Size, Vec2f, Vec3f, Vec4f};
use core::util::{any_as_u8_slice_array, any_as_x_slice_array};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::{
    fs::File,
    sync::{mpsc, Arc, Mutex},
};
use taskpool::{TaskPool, TaskPoolBuilder};

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
struct MaterialMap {
    pub map: HashMap<(Option<usize>, MaterialInputKind), Arc<Material>>,
    pub part_map: Option<HashMap<Option<usize>, (MaterialFaceBuilder, MaterialBuilder)>>,
    context: Option<RContextRef>,
}

impl MaterialMap {
    pub fn new(c: RContextRef) -> Self {
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
            m.clone().with_face(b.build()).build(&context)
        });
        self.part_map = Some(part_map);
        self.context = Some(context);
        k.clone()
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

    let image = image::load_from_memory(&data)?;
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
struct LoadResult {
    total_nodes: u64,
    total_meshes: u64,
    total_vertices: u64,
    total_indices: u64,
    total_textures: u64,
    total_samplers: u64,
    aabb: BoundBox,
}

struct GltfBufferView<'a> {
    buffer: Vec<GltfDataViewSource<'a>>,
    texture: Vec<GltfDataViewSource<'a>>,
}

pub struct ParseContext<'a> {
    pool: &'a TaskPool,
    gpu: Arc<WGPUResource>,
    ctx: RContextRef,
    // opened_files: HashMap<Box<File>>,
    scene: Scene,
    res: LoadResult,
}

fn parse_primitive_indices(
    p: &gltf::Primitive,
    mesh_builder: &mut MeshBuilder,
    buf_view: &GltfBufferView,
    res: &mut LoadResult,
) -> anyhow::Result<()> {
    if let Some(indices) = p.indices() {
        match indices.dimensions() {
            gltf::accessor::Dimensions::Scalar => {}
            _ => {
                anyhow::bail!("dimension for indices invalid");
            }
        }

        let mut kind = MaterialInputKind::None;

        for (semantic, _) in p.attributes() {
            match semantic {
                gltf::Semantic::Colors(_) => {
                    mesh_builder.add_property(MeshPropertyType::Color);
                }
                gltf::Semantic::TexCoords(_) => {
                    mesh_builder.add_property(MeshPropertyType::TexCoord);
                }
                _ => (),
            }
        }

        match indices.data_type() {
            gltf::accessor::DataType::U8 => {
                let buf = buf_view.buffer[0].read_bytes_from_accessor(&indices);
                let mut input = Vec::new();
                for d in any_as_x_slice_array::<u8, _>(&buf) {
                    input.push(*d as u32);
                }
                res.total_indices += (buf.len() / std::mem::size_of::<u8>()) as u64;
                mesh_builder.add_indices32(&input);
            }
            gltf::accessor::DataType::U16 => {
                let buf = buf_view.buffer[0].read_bytes_from_accessor(&indices);

                let mut input = Vec::new();
                for d in any_as_x_slice_array::<u16, _>(&buf) {
                    input.push(*d as u32);
                }
                res.total_indices += (buf.len() / std::mem::size_of::<u16>()) as u64;
                mesh_builder.add_indices32(&input);
            }
            gltf::accessor::DataType::U32 => {
                let buf = buf_view.buffer[0].read_bytes_from_accessor(&indices);
                mesh_builder.add_indices32(any_as_x_slice_array(&buf));
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

fn parse_primitive_vertices(
    p: &gltf::Primitive,
    mesh_builder: &mut MeshBuilder,
    buf_view: &GltfBufferView,
    res: &mut LoadResult,
) -> anyhow::Result<MaterialInputKind> {
    let mut kind = MaterialInputKind::None;

    for (semantic, accessor) in p.attributes() {
        match semantic {
            gltf::Semantic::Extras(ext) => {
                log::info!("extra {}", ext);
            }
            gltf::Semantic::Positions => {
                let buf = buf_view.buffer[0].read_bytes_from_accessor(&accessor);
                match accessor.data_type() {
                    gltf::accessor::DataType::F32 => {}
                    _ => {
                        anyhow::bail!("position invalid data type");
                    }
                };
                match accessor.dimensions() {
                    gltf::accessor::Dimensions::Vec3 => {
                        let data: &[Vec3f] = any_as_x_slice_array(buf);
                        res.total_vertices += data.len() as u64;
                        mesh_builder.add_position_vertices3(data);
                    }
                    _ => {
                        anyhow::bail!("position should be vec3f");
                    }
                };
            }
            gltf::Semantic::Normals => {}
            gltf::Semantic::Tangents => {}
            gltf::Semantic::Colors(_index) => {
                kind = kind.add_color().unwrap();

                let buf = buf_view.buffer[0].read_bytes_from_accessor(&accessor);
                match accessor.data_type() {
                    gltf::accessor::DataType::F32 => {}
                    _ => {
                        anyhow::bail!("color invalid data type");
                    }
                };
                match accessor.dimensions() {
                    gltf::accessor::Dimensions::Vec4 => {
                        let data: &[Vec4f] = any_as_x_slice_array(buf);
                        mesh_builder.add_property_vertices(MeshPropertyType::Color, data);
                    }
                    gltf::accessor::Dimensions::Vec3 => {
                        let data: &[Vec3f] = any_as_x_slice_array(buf);
                        let mut trans_data = Vec::new();
                        for block in data {
                            trans_data.push(Vec4f::new(block[0], block[1], block[2], 1f32));
                        }
                        mesh_builder.add_property_vertices(MeshPropertyType::Color, &trans_data);
                    }
                    _ => {
                        anyhow::bail!("color should be vec3f/vec4f");
                    }
                };
            }
            gltf::Semantic::TexCoords(_index) => {
                if let Some(v) = kind.add_texture() {
                    kind = v;
                }

                let buf = buf_view.buffer[0].read_bytes_from_accessor(&accessor);
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

                mesh_builder.add_property_vertices(MeshPropertyType::TexCoord, &data);
            }
            gltf::Semantic::Joints(_index) => {}
            gltf::Semantic::Weights(_index) => {}
        }
    }
    Ok(kind)
}

impl<'a> ParseContext<'a> {
    fn parse_mesh(
        &mut self,
        tag_id: TagId,
        buf_view: &mut GltfBufferView<'a>,
        mesh: gltf::Mesh,
        material_map: &mut MaterialMap,
        transform: &Transform,
    ) -> anyhow::Result<BoundBox> {
        self.res.total_meshes += 1;

        let mut bound_box = BoundBox::default();
        for p in mesh.primitives() {
            let bb = p.bounding_box();
            let mut bb = BoundBox::new(bb.min.into(), bb.max.into());
            bb.mul_mut(transform.mat());

            bound_box = &bound_box + &bb;

            let mut mesh_builder = MeshBuilder::new();

            parse_primitive_indices(&p, &mut mesh_builder, buf_view, &mut self.res)?;
            let kind = parse_primitive_vertices(&p, &mut mesh_builder, buf_view, &mut self.res)?;

            let color: Color = p
                .material()
                .pbr_metallic_roughness()
                .base_color_factor()
                .into();

            let idx = p.material().index();

            let material = material_map.prepare_kind(idx, kind);
            let mut g = StaticGeometry::new(Arc::new(mesh_builder.build()?));
            g.set_attribute(core::mesh::Attribute::ConstantColor, Arc::new(color));
            g = g.with_transform(transform.clone());
            let mut obj = RenderObject::new(Box::new(g), material);
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
        material_map: &mut MaterialMap,
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
            let bb = self.parse_mesh(tag_id, buf, mesh, material_map, &transform_node)?;
            self.res.aabb = &self.res.aabb + &bb;
        }
        self.res.total_nodes += 1;

        for node in node.children() {
            self.parse_node(node, tag_id, buf, material_map, &transform_node)?;
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
    ) -> anyhow::Result<MaterialMap> {
        let mut map = MaterialMap::new(self.ctx.clone());
        // add default material
        {
            let primitive = wgpu::PrimitiveState::default();
            let mut material_builder = MaterialBuilder::default();
            material_builder = material_builder.with_primitive(primitive);
            material_builder = material_builder.with_name("default");

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
            material_builder = material_builder.with_name(material.name().unwrap_or_default());

            let mut basic_material_builder = BasicMaterialFaceBuilder::default();

            let color = material.pbr_metallic_roughness().base_color_factor();
            let texture = material.pbr_metallic_roughness().base_color_texture();
            if let Some(tex) = texture {
                let texture_index = tex.texture().index();
                let (sampler_index, texture) = texture_map.get(&texture_index).unwrap();
                basic_material_builder = basic_material_builder.with_texture_data(texture.clone());
                if let Some(index) = sampler_index {
                    basic_material_builder =
                        basic_material_builder.with_sampler(samplers[*index].clone());
                } else {
                    // use default
                    basic_material_builder =
                        basic_material_builder.with_sampler(self.gpu.default_sampler());
                }
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
        Ok(map)
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
                            GltfDataViewSource::new_cursor(SourcePosition { size, offset }, &blob)
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
            self.res.total_textures += 1;
        }

        let mut samplers = vec![];
        for sampler in gltf.samplers() {
            let sampler = parse_sampler(&sampler, &self.gpu);
            samplers.push(sampler);
            self.res.total_samplers += 1;
        }

        batch.wait()?;

        let texture_map = texture_map.into_inner()?;
        Ok((texture_map, samplers))
    }

    fn load_meshes(
        &mut self,
        gltf: &gltf::Gltf,
        buf_view: &mut GltfBufferView<'a>,
        material_map: &mut MaterialMap,
    ) -> anyhow::Result<()> {
        for s in gltf.scenes() {
            let name = s.name().unwrap_or("gltf-scene");
            let tag_id = self.scene.context().new_tag(name);

            let transform = Transform::default();
            for node in s.nodes() {
                self.parse_node(node, tag_id, buf_view, material_map, &transform)?;
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

        let bound_box = &mut self.res.aabb;

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
    ) -> anyhow::Result<(Scene, LoadResult)> {
        let gltf = gltf::Gltf::open(&path)?;
        let mut this = Self {
            scene: Scene::new(ctx.clone()),
            res: LoadResult::default(),
            gpu,
            ctx,
            pool,
        };
        let mut buf_view = this.load_buffers(&gltf, path)?;

        let (textures, samplers) = this.load_textures(&gltf, &buf_view)?;
        let mut material_map = this.load_materials(&gltf, &textures, &samplers)?;

        this.load_meshes(&gltf, &mut buf_view, &mut material_map)?;
        this.load_cameras(&gltf)?;

        Ok((this.scene, this.res))
    }
}

pub struct LoadRes {
    pub name: String,
    pub scene: Option<Arc<Scene>>,
    pub error_string: String,
}

fn loader_main(
    rx: mpsc::Receiver<(String, Arc<WGPUResource>)>,
    tx: Arc<Mutex<VecDeque<LoadRes>>>,
    ctx: RContextRef,
) {
    let pool = TaskPoolBuilder::new().build();
    loop {
        let (name, gpu) = rx.recv().unwrap();
        if name.is_empty() {
            break;
        }

        let path = PathBuf::from(name.clone());
        let result = ParseContext::load(&path, &pool, ctx.clone(), gpu);
        let (scene, result) = match result {
            Ok(val) => val,
            Err(err) => {
                log::error!("{} in {}", err, name);
                tx.lock().unwrap().push_back(LoadRes {
                    name,
                    scene: None,
                    error_string: format!("{}", err),
                });
                continue;
            }
        };

        log::info!("load model {} {:?}", name, result);
        tx.lock().unwrap().push_back(LoadRes {
            name,
            scene: Some(Arc::new(scene)),
            error_string: String::new(),
        });
    }
}

pub struct Loader {
    thread: Option<std::thread::JoinHandle<()>>,
    tx: mpsc::Sender<(String, Arc<WGPUResource>)>,
    result: Arc<Mutex<VecDeque<LoadRes>>>,
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

    pub fn load_async<S: Into<String>>(&self, name: S) {
        let _ = self
            .tx
            .send((name.into(), self.gpu.lock().unwrap().clone().unwrap()));
    }

    fn take_result(&self) -> Vec<LoadRes> {
        let mut result = self.result.lock().unwrap();
        if result.is_empty() {
            return vec![];
        }
        let mut tmp = VecDeque::new();
        std::mem::swap(&mut tmp, &mut *result);

        return tmp.into_iter().collect();
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
                core::event::Event::FirstSync => {
                    let mut gpu = self.loader.gpu.lock().unwrap();
                    if gpu.is_none() {
                        let g = context.container.get::<WGPUResource>().unwrap();
                        *gpu = Some(g);
                    }
                }
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
    Loaded(LoadRes),
    Unknown,
}
