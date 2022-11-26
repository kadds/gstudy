use winit::event_loop::EventLoopProxy;

use crate::event::Event;
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
use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
    sync::{mpsc, Arc, Mutex},
};

fn load(name: &str) -> anyhow::Result<Scene> {
    let gltf = gltf::Gltf::open(name)?;
    let mut buf_readers: Vec<Box<dyn Read>> = Vec::new();
    for buf in gltf.buffers() {
        buf_readers.push(match buf.source() {
            gltf::buffer::Source::Bin => {
                Box::new(std::io::Cursor::new(gltf.blob.as_ref().unwrap()))
            }
            gltf::buffer::Source::Uri(uri) => Box::new(BufReader::new(File::open(uri).unwrap())),
        });
    }
    let mut gscene = Scene::new();

    for s in gltf.scenes() {
        for node in s.nodes() {
            if let Some(mesh) = node.mesh() {
                let mut gmesh = Mesh::new();
                let mut gmaterial = Arc::new(BasicMaterial::new(BasicMaterialParameter::new()));
                for p in mesh.primitives() {
                    let indices = p.indices().unwrap();
                    match indices.data_type() {
                        gltf::accessor::DataType::U16 => {}
                        gltf::accessor::DataType::U32 => {}
                        _ => {
                            anyhow::bail!("data tyep for indices is not supported")
                        }
                    }
                    let idx = indices.index();
                }
                let mut g = StaticGeometry::new(Arc::new(gmesh));
                gscene.add_object(Object::new(Box::new(g), gmaterial));
            }
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
