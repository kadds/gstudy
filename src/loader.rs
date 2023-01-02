use winit::event_loop::EventLoopProxy;

use crate::core::backends::wgpu_backend::{WGPURenderTarget, WGPUResource};
use crate::core::backends::WGPUBackend;
use crate::core::context::{RContext, RContextRef};
use crate::core::ps::PrimitiveStateDescriptor;
use crate::event::Event;
use crate::geometry::axis::{Axis, AxisMesh};
use crate::geometry::{Geometry, MeshCoordType};
use crate::render::material::basic::{BasicMaterialFaceBuilder, BasicMaterialShader};
use crate::render::material::MaterialBuilder;
use crate::render::scene::{LAYER_BACKGROUND, LAYER_NORMAL, LAYER_TRANSPARENT};
use crate::render::Material;
use crate::types::{Vec3f, Vec4f};
use crate::util::{any_as_u8_slice_array, any_as_x_slice_array};
use crate::{
    event::{CustomEvent, EventProcessor, EventSource, ProcessEventResult},
    geometry::{BasicGeometry, Mesh, StaticGeometry},
    model::Model,
    render::{scene::Object, Scene},
};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Seek, SeekFrom};
use std::path::PathBuf;
use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
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
                c.seek(SeekFrom::Start(offset));
                let mut buf = Vec::new();
                buf.resize(size as usize, 0);
                c.read_exact(&mut buf).unwrap();
                buf
            }
            GltfBuffer::File(f) => {
                f.seek(SeekFrom::Start(offset));
                let mut buf = Vec::new();
                buf.resize(size as usize, 0);
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
    pub map: HashMap<(usize, MaterialInputKind), Arc<Material>>,
    pub part_map: Option<HashMap<usize, (MaterialFaceBuilder, MaterialBuilder)>>,
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

    pub fn prepare_kind(&mut self, idx: usize, kind: MaterialInputKind) -> Arc<Material> {
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
    buf_readers: &mut Vec<GltfBuffer>,
    mesh: gltf::Mesh,
    material_map: &mut MaterialMap,
) -> anyhow::Result<()> {
    for p in mesh.primitives() {
        let mut gmesh = Mesh::new();
        let mut color = Vec4f::new(1.0f32, 1.0f32, 1.0f32, 1.0f32);
        let indices = p.indices().unwrap();
        match indices.data_type() {
            gltf::accessor::DataType::U16 => {
                match indices.dimensions() {
                    gltf::accessor::Dimensions::Scalar => {}
                    _ => {
                        anyhow::bail!("dimension for indices invalid");
                    }
                }
                indices.view().unwrap().buffer();
                let buf = buf_readers[0].read_bytes(&indices);
                let mut input = Vec::new();
                for d in any_as_x_slice_array::<u16, _>(&buf) {
                    input.push(*d as u32);
                }
                drop(buf);
                gmesh.add_indices(&input);
            }
            gltf::accessor::DataType::U32 => {
                match indices.dimensions() {
                    gltf::accessor::Dimensions::Scalar => {}
                    _ => {
                        anyhow::bail!("dimension for indices invalid");
                    }
                }
                let buf = buf_readers[0].read_bytes(&indices);
                gmesh.add_indices(any_as_x_slice_array(&buf));
            }
            _ => {
                anyhow::bail!("data type for indices is not supported")
            }
        }

        let mut kind = MaterialInputKind::None;

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

                    gmesh.add_vertices(&data);
                }
                gltf::Semantic::Normals => {
                    // param.has_normal = true;
                }
                gltf::Semantic::Tangents => {}
                gltf::Semantic::Colors(index) => {
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

                    gmesh.set_coord_vec4f(MeshCoordType::Color, data);
                }
                gltf::Semantic::TexCoords(index) => {
                    kind = kind.add_texture().unwrap();
                }
                gltf::Semantic::Joints(index) => {}
                gltf::Semantic::Weights(index) => {}
            }
        }
        color = p
            .material()
            .pbr_metallic_roughness()
            .base_color_factor()
            .into();

        let idx = p.material().index().unwrap();

        let material = material_map.prepare_kind(idx, kind);
        let mut g = StaticGeometry::new(Arc::new(gmesh));
        g.set_attribute(crate::geometry::Attribute::ConstantColor, Arc::new(color));

        gscene.add_object(Object::new(Box::new(g), material));
    }

    Ok(())
}

fn parse_node(
    gscene: &mut Scene,
    node: gltf::Node,
    buf_readers: &mut Vec<GltfBuffer>,
    material_map: &mut MaterialMap,
) -> anyhow::Result<()> {
    if let Some(mesh) = node.mesh() {
        parse_mesh(gscene, buf_readers, mesh, material_map)?;
    }

    for node in node.children() {
        parse_node(gscene, node, buf_readers, material_map)?;
    }
    Ok(())
}

fn load(name: &str, rm: Arc<ResourceManager>) -> anyhow::Result<Scene> {
    let path = PathBuf::from(name);
    let gltf = gltf::Gltf::open(&path)?;
    let mut buf_readers: Vec<GltfBuffer> = Vec::new();
    for buf in gltf.buffers() {
        buf_readers.push(match buf.source() {
            gltf::buffer::Source::Bin => {
                GltfBuffer::Cursor(std::io::Cursor::new(gltf.blob.as_ref().unwrap()))
            }
            gltf::buffer::Source::Uri(uri) => {
                let target = path.parent().unwrap().join(uri);
                log::info!("read gltf uri {}", uri);
                GltfBuffer::File(BufReader::new(File::open(target).unwrap()))
            }
        });
    }
    let mut gscene = rm.new_scene();
    let mut map = MaterialMap::new(rm.gpu.context());

    for material in gltf.materials() {
        let idx = material.index().unwrap_or_default();
        let mut primitive = PrimitiveStateDescriptor::default();
        let mut material_builder = MaterialBuilder::default();
        if material.double_sided() {
            primitive = primitive.with_cull_face(crate::core::ps::CullFace::None);
        }

        let mut basic_material_builder = BasicMaterialFaceBuilder::default();

        let color = material.pbr_metallic_roughness().base_color_factor();

        basic_material_builder = basic_material_builder.with_constant_color(color.into());

        map.part_map.as_mut().unwrap().insert(
            idx,
            (
                MaterialFaceBuilder::Basic(basic_material_builder),
                material_builder,
            ),
        );
    }

    for s in gltf.scenes() {
        for node in s.nodes() {
            parse_node(&mut gscene, node, &mut buf_readers, &mut map)?;
        }
        log::info!(
            "model scene {} nodes {}",
            s.name().unwrap_or_default(),
            s.nodes().len()
        );
    }
    let camera = match gltf.cameras().next() {
        Some(c) => {
            log::info!("scene camera {}", c.name().unwrap_or_default());
            match c.projection() {
                gltf::camera::Projection::Orthographic(c) => {
                    let camera = crate::render::Camera::new();
                    camera.make_orthographic(
                        Vec4f::new(c.xmag(), c.ymag(), c.xmag(), c.ymag()),
                        c.znear(),
                        c.zfar(),
                    );
                    camera
                }
                gltf::camera::Projection::Perspective(c) => {
                    let camera = crate::render::Camera::new();
                    camera.make_perspective(
                        c.aspect_ratio().unwrap_or(1.0f32),
                        c.yfov(),
                        c.znear(),
                        c.zfar().unwrap_or(1000_000f32),
                    );
                    camera
                }
            }
        }
        None => {
            let camera = crate::render::Camera::new();
            camera.make_perspective(1.0f32, std::f32::consts::FRAC_PI_3, 0.01f32, 1000_000f32);
            camera
        }
    };
    camera.look_at(
        Vec3f::new(30f32, 15f32, 30f32).into(),
        Vec3f::new(0f32, 0f32, 0f32).into(),
        Vec3f::new(0f32, 1f32, 0f32),
    );
    let camera = Arc::new(camera);

    gscene.set_layer_camera(LAYER_NORMAL, camera.clone());
    gscene.set_layer_camera(LAYER_BACKGROUND, camera.clone());
    gscene.set_layer_camera(LAYER_TRANSPARENT, camera.clone());

    Ok(gscene)
}

fn loader_main(rx: mpsc::Receiver<(String, EventLoopProxy<Event>)>, rm: Arc<ResourceManager>) {
    loop {
        let (name, proxy) = rx.recv().unwrap();
        if name.is_empty() {
            break;
        }
        let result = load(&name, rm.clone());
        let result = match result {
            Ok(val) => val,
            Err(err) => {
                log::error!("{} in {}", err, name);
                continue;
            }
        };

        log::error!("load model {}", name);
        let _ = proxy.send_event(Event::CustomEvent(CustomEvent::Loaded(rm.insert(result))));
    }
}

pub struct Loader {
    thread: Option<std::thread::JoinHandle<()>>,
    tx: mpsc::Sender<(String, EventLoopProxy<Event>)>,
    resource_manager: Arc<ResourceManager>,
}

impl Loader {
    pub fn new(resource_manager: Arc<ResourceManager>) -> Self {
        let (tx, rx) = mpsc::channel();
        let rm = resource_manager.clone();
        let mut this = Self {
            thread: None.into(),
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
    tx: mpsc::Sender<(String, EventLoopProxy<Event>)>,
}

impl EventProcessor for LoaderEventProcessor {
    fn on_event(&mut self, source: &dyn EventSource, event: &Event) -> ProcessEventResult {
        match event {
            Event::CustomEvent(e) => match e {
                CustomEvent::Loading(name) => {
                    let _ = self.tx.send((name.clone(), source.event_proxy()));
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
