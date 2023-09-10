use core::backends::wgpu_backend::WGPUResource;
use core::context::{RContext, ResourceRef};
use core::event::EventProcessor;
use core::graph::rdg::resource::RT_COLOR_RESOURCE_ID;
use core::graph::rdg::{RenderGraph, RenderGraphBuilder};
use core::render::{HardwareRenderer, ModuleRenderer, RenderParameter, SetupConfig};
use core::scene::controller::CameraControllerFactory;
use core::scene::Scene;
use core::types::{Color, Size, Vec4f};
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub mod looper;
pub mod statistics;
pub mod window;
use app::container::{Container, LockResource};
use app::plugin::{LooperPlugin, Plugin, PluginFactory};
use app::AppEventProcessor;
pub use looper::Looper;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use statistics::Statistics;
pub use winit;
mod util;

pub type DEvent = Box<dyn Any + Send>;
pub type WEvent<'a> = winit::event::Event<'a, DEvent>;
pub type CEvent = core::event::Event;

struct HardwareRenderPlugin {
    rdg: Option<RenderGraph>,
    renderer: HardwareRenderer,
    cc_factory: Option<Arc<CameraControllerFactory>>,
    first_update: bool,
}

impl HardwareRenderPlugin {
    pub fn new() -> Self {
        Self {
            rdg: None,
            renderer: HardwareRenderer::new(),
            first_update: true,
            cc_factory: None,
        }
    }
    fn update(&mut self, _delta: f32) {
        if self.first_update {
            self.first_update = false;
            log::info!("App startup");
        }
    }

    fn render(&mut self, texture: ResourceRef, container: &Container) {
        profiling::scope!("hardware render body");
        let gpu = container.get::<WGPUResource>().unwrap();
        let (view_size, logic_size) = container.get::<WindowSize>().unwrap().get();

        let clear_color = container.get::<ClearColor>().unwrap().get();
        let scene = container.get::<Scene>().unwrap();
        if scene.material_change() {
            self.rdg = None;
        }
        if scene.has_rebuild_flag() {
            self.rdg = None;
        }

        scene.ui_camera_ref().make_orthographic(
            Vec4f::new(0f32, 0f32, logic_size.x as f32, logic_size.y as f32),
            0.1f32,
            10f32,
        );

        if self.rdg.is_none() {
            let msaa = container.get::<MsaaResource>().unwrap();
            let mut graph_builder = RenderGraphBuilder::new("main graph");
            let aa = msaa.get().0;
            graph_builder.set_msaa(aa);

            let config = SetupConfig { msaa: aa };

            self.renderer
                .setup(&mut graph_builder, gpu.clone(), &scene, &config);
            log::info!("rebuild render graph with view size {:?}", view_size);
            // container.get::<RContext>().unwrap();
            let real_size = Size::new(
                texture.texture_ref().width(),
                texture.texture_ref().height(),
            );

            graph_builder.set_present_target(real_size, gpu.surface_format(), Some(clear_color));
            self.rdg = Some(graph_builder.compile());
            scene.clear_rebuild_flag();
        }

        self.rdg
            .as_mut()
            .unwrap()
            .registry()
            .import(RT_COLOR_RESOURCE_ID, texture);

        let p = RenderParameter {
            gpu: gpu.clone(),
            scene: scene.clone(),
            g: self.rdg.as_mut().unwrap(),
        };

        self.renderer.render(p);
    }
}

impl Plugin for HardwareRenderPlugin {
    fn install_factory(
        &mut self,
        container: &Container,
        factory_list: &mut app::plugin::CoreFactoryList,
    ) {
        let mut list = app::plugin::CoreFactoryList::default();
        std::mem::swap(&mut list, factory_list);

        for (face_id, factory) in list.materials {
            self.renderer.add_factory(face_id, factory);
        }

        let mut cc = CameraControllerFactory::new();

        for (name, factory) in list.camera_controllers {
            cc.add(name, factory);
        }

        container.register(cc);

        self.cc_factory = container.get::<_>();
    }
}

impl AppEventProcessor for HardwareRenderPlugin {
    fn on_event(&mut self, context: &app::AppEventContext, event: &dyn Any) {
        if let Some(ev) = event.downcast_ref::<CEvent>() {
            match ev {
                core::event::Event::Update(delta) => {
                    self.update(*delta as f32);
                }
                core::event::Event::Render(texture) => {
                    self.render(texture.clone(), context.container);
                }
                core::event::Event::Resized { logical, physical } => {
                    self.rdg = None;
                    context.container.get::<WindowSize>().unwrap().set((
                        Size::new(physical.x, physical.y),
                        Size::new(logical.x, logical.y),
                    ));
                }
                _ => (),
            }
        }
    }
}

#[derive(Default)]
pub struct HardwareRenderPluginFactory;

impl PluginFactory for HardwareRenderPluginFactory {
    fn create(&self, _container: &Container) -> Box<dyn Plugin> {
        Box::new(HardwareRenderPlugin::new())
    }

    fn info(&self) -> app::plugin::PluginInfo {
        app::plugin::PluginInfo {
            name: "HardwareRenderPlugin".into(),
            version: "0.1.0".into(),
            has_looper: false,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum Theme {
    Light,
    Dark,
}

#[derive(Debug)]
pub enum Event {
    UpdateCursor(winit::window::CursorIcon),

    UpdateImePosition((u32, u32)),

    ScaleFactorChanged(f64),

    Theme(Theme),

    Moved(Size),

    CloseRequested,

    Exit,

    Focused(bool),

    FullScreen(bool),

    OpenUrl(String),
}

pub struct WindowPluginFactory {
    title: String,
    size: Size,
}

impl WindowPluginFactory {
    pub fn new<T: Into<String>>(title: T, size: Size) -> Self {
        Self {
            title: title.into(),
            size,
        }
    }
}

impl PluginFactory for WindowPluginFactory {
    fn create(&self, _container: &Container) -> Box<dyn app::plugin::Plugin> {
        Box::new(WindowPlugin)
    }

    fn info(&self) -> app::plugin::PluginInfo {
        app::plugin::PluginInfo {
            name: "window".into(),
            version: "0.1.0".into(),
            has_looper: true,
        }
    }

    fn create_looper(&self, _container: &Container) -> Option<Box<dyn app::plugin::LooperPlugin>> {
        Some(Box::new(WindowLooperPlugin {
            title: self.title.clone(),
            size: self.size,
        }))
    }
}

pub struct RawWindow {
    pub handle: RawWindowHandle,
}

unsafe impl HasRawWindowHandle for RawWindow {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.handle
    }
}

unsafe impl Send for RawWindow {}
unsafe impl Sync for RawWindow {}

pub type WindowSize = LockResource<(Size, Size)>;
pub type ClearColor = LockResource<Color>;
pub type MainWindowHandle = RawWindow;

pub type StatisticsResource = Mutex<Statistics>;

#[derive(Default, Clone, Debug, Copy)]
pub struct Msaa(pub u32);

pub type MsaaResource = LockResource<Msaa>;

pub struct WindowPlugin;

impl Plugin for WindowPlugin {}

pub struct WindowLooperPlugin {
    title: String,
    size: Size,
}

impl LooperPlugin for WindowLooperPlugin {
    fn run(
        &self,
        container: &app::container::Container,
        runner: Rc<RefCell<dyn app::plugin::Runner>>,
    ) {
        let mut looper = Looper::new();
        let window_builder = winit::window::WindowBuilder::new()
            .with_inner_size(winit::dpi::Size::Logical(winit::dpi::LogicalSize::new(
                self.size.x as f64,
                self.size.y as f64,
            )))
            .with_resizable(true)
            .with_visible(false)
            .with_title(self.title.clone());

        container.register(WindowSize::new((self.size, self.size)));
        container.register(ClearColor::new(Color::new(0.1f32, 0.1f32, 0.1f32, 1f32)));

        let context = container.get::<RContext>().unwrap();

        let gpu = looper.create_window(window_builder, context);

        container.register(RawWindow {
            handle: looper.handle().unwrap(),
        });
        container.register_arc(gpu);
        container.register(MsaaResource::new(Msaa(1)));
        container.register_arc(looper.statistics());

        struct Process(Rc<RefCell<dyn app::plugin::Runner>>);

        impl EventProcessor for Process {
            fn on_event(
                &mut self,
                source: &dyn core::event::EventSource,
                event: &dyn Any,
            ) -> core::event::ProcessEventResult {
                self.0.borrow_mut().on_event(source, event)
            }
        }
        looper
            .event_registry()
            .register_processor(Box::new(Process(runner.clone())));

        runner.borrow().startup(&looper.event_source());
        looper.run();
    }
}

impl AppEventProcessor for WindowPlugin {
    fn on_event(&mut self, context: &app::AppEventContext, event: &dyn Any) {}
}
