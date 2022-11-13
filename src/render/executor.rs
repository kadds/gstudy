use std::{
    collections::{HashMap, HashSet},
    f32::consts::PI,
    sync::{mpsc, Arc},
    thread, time::Duration,
};

use winit::event::VirtualKeyCode;

use super::{
    camera::{Camera, CameraController, EventController},
    material::{
        BasicMaterial, BasicMaterialParameter, ConstantMaterial, ConstantMaterialParameter,
        DepthMaterial, DepthMaterialParameter,
    },
    scene::Object,
    transform::TransformBuilder,
    Canvas, Scene, Transform,
};
use crate::{
    backends::wgpu_backend::WGPUResource,
    geometry::{
        axis::{Axis, AxisMesh},
        plane::{Plane, PlaneMesh},
        sphere::{Sphere, SphereMesh},
    },
    modules::*,
    types::{Vec2f, Vec3f, Vec4f}, statistics::Statistics,
};

pub type TaskId = u64;

struct TaskProxy {
    tx: mpsc::Sender<TaskOperation>,
    task: Arc<Task>,
}

pub struct Executor {
    modules: Vec<Box<dyn ModuleFactory>>,
    tasks: HashMap<TaskId, TaskProxy>,
    last_task_id: TaskId,
    tasks_to_wakeup: Vec<TaskId>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MouseButton {
    Left,
    Mid,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    MouseMove(Vec2f),
    MouseDrag(Vec2f),
    MouseStartDrag,
    MouseEndDrag,

    MouseDown(MouseButton),
    MouseUp(MouseButton),

    KeyboardDown(VirtualKeyCode),
    KeyboardUp(VirtualKeyCode),
    KeyboardInput(VirtualKeyCode),

    Enable,
    Disable,
}

enum TaskOperation {
    None,
    Pause,
    Resume,
    Start(Arc<WGPUResource>),
    Stop,
    Input(InputEvent),
}

struct Task {
    canvas: Arc<Canvas>,
}

impl Task {
    pub fn new(canvas: Arc<Canvas>) -> Arc<Self> {
        Self { canvas }.into()
    }
    pub fn start(
        self: Arc<Self>,
        info: ModuleInfo,
        renderer: Box<dyn ModuleRenderer>,
        rx: mpsc::Receiver<TaskOperation>,
    ) {
        log::info!("{} task running ", info.name);
        thread::Builder::new()
            .name(info.name.to_string())
            .spawn(move || {
                self.main(renderer, rx);
            })
            .unwrap();
    }

    pub fn main(
        self: Arc<Self>,
        mut renderer: Box<dyn ModuleRenderer>,
        rx: mpsc::Receiver<TaskOperation>,
    ) {
        let camera = Camera::new();
        let mut scene = Scene::new();
        let basic_material = Arc::new(BasicMaterial::new(BasicMaterialParameter::new()));
        let constant_material = Arc::new(ConstantMaterial::new(ConstantMaterialParameter::new()));
        let depth_material = Arc::new(DepthMaterial::new(DepthMaterialParameter::new()));
        let depth_line_material =
            Arc::new(DepthMaterial::new(DepthMaterialParameter { line: true }));
        let basic_line_material = Arc::new(BasicMaterial::new(BasicMaterialParameter {
            has_color: true,
            line: true,
            ..Default::default()
        }));

        let axis = Object::new(
            Box::new(Axis::new(AxisMesh::new())),
            basic_line_material.clone(),
        );
        scene.add_object(axis);

        let ground = Object::new(
            Box::new(
                Plane::new(PlaneMesh::new()).build_transform(
                    TransformBuilder::new()
                        .scale(Vec3f::new(20f32, 1f32, 20f32))
                        .translate(Vec3f::new(0f32, 1f32, 0f32))
                        .build(),
                ),
            ),
            depth_material.clone(),
        );
        scene.add_object(ground);

        let sphere = Object::new(
            Box::new(
                Sphere::new(SphereMesh::new(20, 20)).build_transform(
                    TransformBuilder::new()
                        .translate(Vec3f::new(1f32, 10f32, 0f32))
                        .build(),
                ),
            ),
            depth_material,
        );

        // scene.add_object(sphere);

        let mut ctr = Box::new(EventController::new(&camera));

        // camera.make_orthographic(Vec4f::new(0f32, 0f32, 40f32, 40f32), 0.001f32, 100f32);
        camera.make_perspective(1.0f32, PI / 2.0f32 * 0.8f32, 0.1f32, 100f32);
        camera.look_at(
            Vec3f::new(30f32, 15f32, 30f32).into(),
            Vec3f::new(0f32, 0f32, 0f32).into(),
            Vec3f::new(0f32, 1f32, 0f32),
        );
        let mut pause = true;
        let mut stop = false;
        let mut gpu: Option<Arc<WGPUResource>> = None;
        let mut statistics = Statistics::new(Duration::from_secs(1), Some(1f32 / 60f32));

        while !stop {
            // do something
            if !pause && gpu.is_some() {
                let parameter = RenderParameter {
                    gpu: gpu.as_ref().unwrap().clone(),
                    camera: &camera,
                    scene: &scene,
                    canvas: &self.canvas,
                };
                if statistics.new_frame() {
                    renderer.render(parameter);
                }
            }
            let (t, dur, _) = statistics.next_frame();
            std::thread::sleep(dur);
            let mut op = TaskOperation::None;

            if pause {
                if let Ok(tmp_op) = rx.recv() {
                    op = tmp_op;
                }
            } else {
                if let Ok(tmp_op) = rx.try_recv() {
                    op = tmp_op;
                }
            }
            match op {
                TaskOperation::Resume => {
                    pause = false;
                }
                TaskOperation::Pause => {
                    pause = true;
                }
                TaskOperation::Start(gpu_tmp) => {
                    gpu = Some(gpu_tmp.new_queue());
                    pause = false
                }
                TaskOperation::Stop => {
                    stop = true;
                }
                TaskOperation::Input(ev) => ctr.on_input(ev),
                _ => (),
            }
        }
    }
}

impl Drop for Executor {
    fn drop(&mut self) {
        self.stop_all();
    }
}

impl Executor {
    pub fn new() -> Self {
        let mut modules: Vec<Box<dyn ModuleFactory>> = vec![];
        modules.push(Box::new(HardwareRendererFactory::new()));
        modules.push(Box::new(SoftwareRendererFactory::new()));
        modules.push(Box::new(RayTracingFactory::new()));
        Self {
            modules,
            tasks: HashMap::new(),
            last_task_id: 0,
            tasks_to_wakeup: Vec::new(),
        }
    }

    pub fn run(&mut self, index: usize, canvas: Arc<Canvas>) -> TaskId {
        log::info!("click to run");

        let factory = self.modules[index].as_ref();
        let (tx, rx) = mpsc::channel();
        let task = Task::new(canvas);
        task.clone()
            .start(factory.info(), factory.make_renderer(), rx);
        let id = self.last_task_id;
        self.tasks.insert(id, TaskProxy { tx, task });
        self.tasks_to_wakeup.push(id);
        self.last_task_id += 1;
        id
    }

    pub fn stop(&mut self, task_id: TaskId) {
        if let Some(v) = self.tasks.get(&task_id) {
            let _ = v.tx.send(TaskOperation::Stop);
        }
    }

    pub fn stop_all(&mut self) {
        for task in self.tasks.values() {
            let _ = task.tx.send(TaskOperation::Stop);
        }
    }

    pub fn pause(&self, task_id: TaskId) {
        if let Some(v) = self.tasks.get(&task_id) {
            let _ = v.tx.send(TaskOperation::Pause);
        }
    }

    pub fn resume(&self, task_id: TaskId) {
        if let Some(v) = self.tasks.get(&task_id) {
            let _ = v.tx.send(TaskOperation::Resume);
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

    pub fn module_list(&self) -> Vec<ModuleInfo> {
        self.modules.iter().map(|it| it.info()).collect()
    }

    pub fn tasks(&self) -> HashSet<TaskId> {
        self.tasks.keys().into_iter().copied().collect()
    }

    pub fn update(&mut self) {}

    pub fn render(&mut self, gpu: Arc<WGPUResource>) {
        for id in &self.tasks_to_wakeup {
            let task = self.tasks.get(id).unwrap();
            if let Err(e) = task.tx.send(TaskOperation::Start(gpu.clone())) {
                log::error!("{}", e)
            }
        }
        self.tasks_to_wakeup.clear();
    }

    pub fn send_input(&self, task_id: TaskId, event: InputEvent) {
        let _ = self
            .tasks
            .get(&task_id)
            .unwrap()
            .tx
            .send(TaskOperation::Input(event));
    }
}
