use std::{
    f32::consts::PI,
    mem::swap,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
};

use super::{
    camera::{Camera, CameraController, EventController},
    material::BasicMaterial,
    scene::Object,
    Canvas, Scene,
};
use crate::{
    backends::wgpu_backend::WGPUResource,
    geometry::plane::Plane,
    modules::*,
    types::{Vec2f, Vec3f, Vec4f},
};

pub struct Executor {
    modules: Vec<Box<dyn ModuleFactory>>,
    current_module: usize,
    new_canvas: Option<Arc<Canvas>>,
    world: Option<mpsc::Sender<WorldOperation>>,
    gpu: Rc<WGPUResource>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum InputEvent {
    MouseMove(),
    MouseInput(),
    TouchInput(),
    KeyboardInput(),
}

enum WorldOperation {
    Pause,
    Resume,
    UpdateCanvas(Arc<Canvas>),
    Stop,
    Input(InputEvent),
}

struct World {
    canvas: Arc<Canvas>,
    pause: bool,
    stop: bool,
    rx: mpsc::Receiver<WorldOperation>,
    gpu: Rc<WGPUResource>,
}

impl World {
    pub fn new(
        canvas: Arc<Canvas>,
        rx: mpsc::Receiver<WorldOperation>,
        gpu: Rc<WGPUResource>,
    ) -> Self {
        Self {
            canvas,
            pause: false,
            stop: false,
            rx,
            gpu,
        }
    }
    pub fn start(self, info: ModuleInfo, renderer: Box<dyn ModuleRenderer>) {
        // unsafe {
        //     log::info!("{} task running ", info.name);
        //     thread::Builder::new()
        //         .name(info.name.to_string())
        //         .spawn_unchecked(|| {
        //             self.main(renderer);
        //         })
        //         .unwrap();
        // }
    }

    fn do_op(&mut self, op: WorldOperation, ctr: &mut dyn CameraController) {
        match op {
            WorldOperation::Resume => {
                self.pause = false;
            }
            WorldOperation::Pause => {
                self.pause = true;
            }
            WorldOperation::UpdateCanvas(canvas) => {
                self.canvas = canvas;
            }
            WorldOperation::Stop => {
                self.stop = true;
            }
            WorldOperation::Input(ev) => {}
        }
    }

    pub fn main(mut self, mut renderer: Box<dyn ModuleRenderer>) {
        let camera = Camera::new();
        let mut scene = Scene::new();
        let basic_material = Arc::new(BasicMaterial::new(Vec4f::new(1f32, 1f32, 0f32, 1f32)));
        let ground = Object::new(
            Box::new(Plane::new(Vec2f::new(10f32, 10f32))),
            basic_material,
        );
        scene.add_object(ground);
        let mut ctr = Box::new(EventController::new(&camera));

        // camera.make_orthographic(Vec4f::new(0f32, 0f32, 40f32, 40f32), 0.001f32, 100f32);
        camera.make_perspective(1.0f32, PI / 2.0f32 * 0.8f32, 0.001f32, 820f32);
        camera.look_at(
            Vec3f::new(0f32, 30f32, 0f32).into(),
            Vec3f::new(0f32, 0f32, 0f32).into(),
            Vec3f::new(0f32, 0f32, 1f32),
        );

        while !self.stop {
            // do something
            if !self.pause {
                let parameter = RenderParameter {
                    gpu: &self.gpu,
                    camera: &camera,
                    scene: &scene,
                    canvas: &self.canvas,
                };
                renderer.render(parameter);
                if let Ok(op) = self.rx.try_recv() {
                    self.do_op(op, ctr.as_mut());
                }
            }

            if self.pause {
                if let Ok(op) = self.rx.recv() {
                    self.do_op(op, ctr.as_mut());
                }
            }
        }
    }
}

impl Drop for Executor {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Executor {
    pub fn new(gpu: Rc<WGPUResource>) -> Self {
        let mut modules: Vec<Box<dyn ModuleFactory>> = Vec::new();
        modules.push(Box::new(HardwareRendererFactory::new()));
        modules.push(Box::new(SoftwareRendererFactory::new()));
        modules.push(Box::new(RayTracingFactory::new()));
        Self {
            modules,
            current_module: usize::MAX,
            new_canvas: None,
            world: None,
            gpu,
        }
    }

    pub fn rerun(&mut self, canvas: Arc<Canvas>) {
        match self.world.as_mut() {
            Some(rx) => {
                let _ = rx.send(WorldOperation::UpdateCanvas(canvas));
            }
            None => (),
        }
    }

    pub fn run(&mut self, name: &str, canvas: Arc<Canvas>) {
        log::info!("click to run");
        let module_index = self.match_module(name);
        let mut idx = self.current_module;
        if idx != usize::MAX {
            self.stop();
        }

        idx = module_index;
        self.current_module = idx;

        let factory = self.modules[idx].as_ref();
        let (tx, rx) = mpsc::channel();
        let task = World::new(canvas, rx, self.gpu.clone());
        self.world = Some(tx);
        task.start(factory.info(), factory.make_renderer());
    }

    pub fn stop(&mut self) {
        log::info!("click to stop");
        match self.world.as_mut() {
            Some(rx) => {
                let _ = rx.send(WorldOperation::Stop);
            }
            None => (),
        }
    }

    pub fn pause(&self) {
        match self.world.as_ref() {
            Some(rx) => {
                let _ = rx.send(WorldOperation::Pause);
            }
            _ => (),
        }
    }

    pub fn resume(&self) {
        match self.world.as_ref() {
            Some(rx) => {
                let _ = rx.send(WorldOperation::Resume);
            }
            _ => (),
        }
    }

    fn match_module(&self, name: &str) -> usize {
        self.modules
            .iter()
            .enumerate()
            .find(|it| it.1.info().name == name)
            .unwrap()
            .0
    }

    pub fn list(&self) -> Vec<ModuleInfo> {
        self.modules.iter().map(|it| it.info()).collect()
    }
}
