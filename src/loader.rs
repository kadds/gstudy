use winit::event_loop::EventLoopProxy;

use crate::event::Event;
use crate::types::Vec3f;
use crate::util::{any_as_f32_slice_array, any_as_u32_slice_array, any_as_u8_slice_array};
use crate::{
    event::{CustomEvent, EventProcessor, EventSource, ProcessEventResult},
    geometry::{BasicGeometry, Mesh, StaticGeometry},
    model::Model,
    render::{
        material::{BasicMaterial, BasicMaterialParameter},
        scene::Object,
        Scene,
    },
};
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
    fn read_bytes(&mut self, offset: usize, size: usize) -> Vec<u8> {
        match self {
            GltfBuffer::Cursor(c) => {
                c.seek(SeekFrom::Start(offset as u64));
                let mut buf = Vec::new();
                buf.resize(size as usize, 0);
                c.read_exact(&mut buf).unwrap();
                buf
            }
            GltfBuffer::File(f) => {
                f.seek(SeekFrom::Start(offset as u64));
                let mut buf = Vec::new();
                buf.resize(size as usize, 0);
                f.read_exact(&mut buf).unwrap();
                buf
            }
        }
    }
}

fn parse_mesh(
    gscene: &mut Scene,
    buf_readers: &mut Vec<GltfBuffer>,
    mesh: gltf::Mesh,
) -> anyhow::Result<()> {
    let mut gmesh = Mesh::new();
    let mut gmaterial = Arc::new(BasicMaterial::new(BasicMaterialParameter::new()));
    for p in mesh.primitives() {
        let indices = p.indices().unwrap();
        match indices.data_type() {
            gltf::accessor::DataType::U16 => {
                anyhow::bail!("not support u16 type");
            }
            gltf::accessor::DataType::U32 => {
                let buf = buf_readers[0].read_bytes(indices.offset(), indices.size());
                gmesh.add_indices(any_as_u32_slice_array(&buf));
            }
            _ => {
                anyhow::bail!("data type for indices is not supported")
            }
        }

        for (semantic, accessor) in p.attributes() {
            match semantic {
                gltf::Semantic::Extras(_) => todo!(),
                gltf::Semantic::Positions => {
                    let buf = buf_readers[0].read_bytes(accessor.offset(), accessor.size());
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
                    let f = any_as_f32_slice_array(&buf);
                    let mut data = Vec::new();
                    for block in f.chunks(3) {
                        data.push(Vec3f::new(block[0], block[1], block[2]));
                    }

                    gmesh.add_vertices(&data);
                }
                gltf::Semantic::Normals => todo!(),
                gltf::Semantic::Tangents => todo!(),
                gltf::Semantic::Colors(_) => todo!(),
                gltf::Semantic::TexCoords(_) => todo!(),
                gltf::Semantic::Joints(_) => todo!(),
                gltf::Semantic::Weights(_) => todo!(),
            }
        }
    }
    let g = StaticGeometry::new(Arc::new(gmesh));
    gscene.add_object(Object::new(Box::new(g), gmaterial));
    Ok(())
}

fn parse_camera(gscene: &mut Scene, node: gltf::Node) {}

fn load(name: &str) -> anyhow::Result<Scene> {
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
                GltfBuffer::File(BufReader::new(File::open(target).unwrap()))
            }
        });
    }
    let mut gscene = Scene::new();

    for s in gltf.scenes() {
        for node in s.nodes() {
            if let Some(mesh) = node.mesh() {
                parse_mesh(&mut gscene, &mut buf_readers, mesh)?;
            }

            parse_camera(&mut gscene, node);
        }
        log::info!(
            "model scene {} nodes {}",
            s.name().unwrap_or_default(),
            s.nodes().len()
        );
    }
    Ok(gscene)
}

fn loader_main(rx: mpsc::Receiver<(String, EventLoopProxy<Event>)>) {
    loop {
        let (name, proxy) = rx.recv().unwrap();
        if name.is_empty() {
            break;
        }
        let result = load(&name);
        let result = match result {
            Ok(val) => val,
            Err(err) => {
                log::error!("{} in {}", err, name);
                continue;
            }
        };

        log::info!("load model {}", name);
        let _ = proxy.send_event(Event::CustomEvent(CustomEvent::Loaded(result)));
    }
}

pub struct Loader {
    thread: Option<std::thread::JoinHandle<()>>,
    tx: mpsc::Sender<(String, EventLoopProxy<Event>)>,
}

impl Loader {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let mut this = Self {
            thread: None.into(),
            tx,
        };
        this.thread = Some(std::thread::spawn(move || {
            loader_main(rx);
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
