use core::backends::wgpu_backend::WGPUResource;
use core::backends::WGPUBackend;
use core::event::{
    CustomEvent, Event, EventProcessor, EventSender, EventSource, InputEvent, ProcessEventResult,
    Theme,
};
use core::geometry::StaticGeometry;
use core::material::egui::EguiMaterialFaceBuilder;
use core::material::{Material, MaterialBuilder};
use core::render::{HardwareRenderer, ModuleRenderer, RenderParameter};
use core::scene::camera::{CameraController, RenderAttachment, TrackballCameraController};
use core::scene::{Camera, Object, Scene, LAYER_NORMAL, LAYER_UI};
use core::texture::Texture;
use core::types::{Size, Vec2f, Vec3f, Vec4f};
use core::ui::{UIMesh, UITextures, UI};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use crate::loader::ResourceManager;
use crate::logic::MainLogic;
use crate::statistics::Statistics;
use crate::util;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{ControlFlow, EventLoop, EventLoopProxy, EventLoopWindowTarget};
use winit::window::Window;
use winit::window::{self, WindowBuilder};
type WEvent<'a> = winit::event::Event<'a, Event>;

struct LooperInner {
    renderer: HardwareRenderer,
    scene: Scene,
    gpu: Arc<WGPUResource>,

    ui_camera: Arc<Camera>,
    ui: UI,

    main_depth_texture: Option<Texture>,

    ui_textures: Option<UITextures>,
    ui_mesh: UIMesh,

    ui_materials: Option<HashMap<egui::TextureId, Arc<Material>>>,
    size: Size,

    controller: Option<Box<dyn CameraController>>,
    last_delta: f32,

    window: Arc<Window>,
    resource_manager: Arc<ResourceManager>,
}

impl LooperInner {
    pub fn new(gpu: Arc<WGPUResource>, window: Arc<Window>, rm: Arc<ResourceManager>) -> Self {
        let mut scene = Scene::new(gpu.context_ref());
        let ui = UI::new(Box::new(MainLogic::new()));
        let ui_camera = Arc::new(Camera::new(gpu.context()));
        ui_camera.make_orthographic(Vec4f::new(0f32, 0f32, 1f32, 1f32), 0.1f32, 10f32);
        ui_camera.look_at(
            Vec3f::new(0f32, 0f32, 1f32),
            Vec3f::zeros(),
            Vec3f::new(0f32, 1f32, 0f32),
        );
        scene.set_layer_camera(LAYER_UI, ui_camera.clone());

        let ui_mesh = UIMesh::new();
        let ui_textures = UITextures::default();

        Self {
            renderer: HardwareRenderer::new(),
            gpu: gpu.clone(),
            scene,
            ui_camera,
            ui,
            main_depth_texture: None,
            size: Size::new(1u32, 1u32),
            ui_materials: Some(HashMap::new()),
            ui_mesh,
            ui_textures: Some(ui_textures),
            controller: None,
            last_delta: 0f32,
            window,
            resource_manager: rm,
        }
    }

    fn update(&mut self, delta: f64) {
        self.last_delta = delta as f32;
        self.scene.update(delta)
    }

    fn build_ui_objects(&mut self) {
        self.scene.clear_layer_objects(LAYER_UI);

        let mut ui_materials = self.ui_materials.take().unwrap();

        let mut ui_textures = self.ui_textures.take().unwrap();
        let meshes =
            self.ui_mesh
                .generate_mesh(&self.ui, self.gpu.clone(), self.size, &mut ui_textures);

        for (mesh, texture_id) in meshes {
            let material = ui_materials.entry(texture_id).or_insert_with(|| {
                let view = ui_textures.get_view(texture_id);
                MaterialBuilder::default()
                    .with_face(
                        EguiMaterialFaceBuilder::default()
                            .with_texture(view)
                            .build(),
                    )
                    .build(self.gpu.context())
            });

            let object = Object::new(
                Box::new(StaticGeometry::new(Arc::new(mesh))),
                material.clone(),
            );

            self.scene.add_ui(object);
        }

        self.ui_textures = Some(ui_textures);
        self.ui_materials = Some(ui_materials);
    }

    fn render(&mut self) {
        self.build_ui_objects();

        let surface_frame = match self.gpu.current_frame_texture() {
            Ok(v) => v,
            Err(e) => {
                log::error!("{}", e);
                return;
            }
        };

        let depth_texture = self.main_depth_texture.as_ref().unwrap();
        let clear_color = self.ui.clear_color();

        let attachment = RenderAttachment::new_with_color_depth(
            0,
            surface_frame.texture(),
            depth_texture.clone(),
            Some(clear_color),
            Some(1f32),
            self.gpu.surface_format(),
        );

        // bind textures
        for (layer, objects) in self.scene.layers() {
            if let Some(c) = objects.camera() {
                c.bind_render_attachment(attachment.clone());
            }
        }

        let p = RenderParameter {
            gpu: self.gpu.clone(),
            scene: &mut self.scene,
        };

        self.renderer.render(p);
    }
}

pub struct Looper {
    backend: Option<Box<WGPUBackend>>,
    event_loop: RefCell<Option<EventLoop<Event>>>,
    event_proxy: EventLoopProxy<Event>,
    processors: Vec<RefCell<Box<dyn EventProcessor>>>,
    frame: Statistics,
    first_render: bool,
    event_sender: DetailEventSender,

    window: Arc<Window>,

    inner: Option<Rc<RefCell<LooperInner>>>,
}

struct DefaultProcessor {
    inner: Rc<RefCell<LooperInner>>,
}

#[derive(Clone)]
pub struct DetailEventSender {
    proxy: EventLoopProxy<Event>,
}

impl EventSender for DetailEventSender {
    fn send_event(&self, ev: Event) {
        self.proxy.send_event(ev).unwrap();
    }
}

impl EventProcessor for DefaultProcessor {
    fn on_event(&mut self, source: &dyn EventSource, event: &Event) -> ProcessEventResult {
        let mut inner = self.inner.borrow_mut();
        match event {
            Event::CloseRequested => {
                return ProcessEventResult::ExitLoop;
            }
            Event::UpdateCursor(c) => {
                if let Some(c) = util::match_winit_cursor(*c) {
                    inner.window.set_cursor_icon(c);
                }
            }
            Event::UpdateImePosition(pos) => {
                inner.window.set_ime_position(winit::dpi::Position::Logical(
                    winit::dpi::LogicalPosition::new(pos.0 as f64, pos.1 as f64),
                ));
            }
            Event::FullScreen(fullscreen) => {
                if *fullscreen {
                    inner
                        .window
                        .set_fullscreen(Some(window::Fullscreen::Borderless(None)));
                } else {
                    inner.window.set_fullscreen(None);
                }
            }
            core::event::Event::Update(delta) => {
                // update egui
                inner.update(*delta);
            }
            core::event::Event::Render => {
                inner.render();
                return core::event::ProcessEventResult::Consumed;
            }
            core::event::Event::Resized { logical, physical } => {
                inner.size = physical.clone();

                // create depth texture
                let texture = inner
                    .gpu
                    .new_depth_texture(Some("depth texture"), physical.clone());

                inner.main_depth_texture = Some(texture);

                inner.ui_camera.make_orthographic(
                    Vec4f::new(0f32, 0f32, logical.x as f32, logical.y as f32),
                    0.1f32,
                    10f32,
                );
                inner.ui_camera.look_at(
                    Vec3f::new(0f32, 0f32, 1f32),
                    Vec3f::zeros(),
                    Vec3f::new(0f32, 1f32, 0f32),
                );
                let aspect = physical.x as f32 / physical.y as f32;
                for (_, layer) in inner.scene.layers() {
                    match layer.camera() {
                        Some(camera) => {
                            if camera.is_perspective() {
                                camera.remake_perspective(aspect);
                            }
                        }
                        None => {}
                    };
                }
            }
            core::event::Event::CustomEvent(ev) => match ev {
                core::event::CustomEvent::Loaded(scene) => {
                    let scene = inner.resource_manager.take(*scene);
                    let mut inner = self.inner.borrow_mut();
                    inner.scene = scene;
                    let cam = inner.ui_camera.clone();
                    inner.scene.set_layer_camera(LAYER_UI, cam);
                    // get camera
                    let camera = inner.scene.layer(LAYER_NORMAL).camera_ref().unwrap();
                    inner.controller = Some(Box::new(TrackballCameraController::new(camera)));
                }
                _ => (),
            },
            core::event::Event::Input(input) => {
                let delta = inner.last_delta;
                if let Some(c) = inner.controller.as_mut() {
                    c.on_input(delta, input.clone());
                }
            }
            _ => (),
        };
        ProcessEventResult::Received
    }
}

impl EventSource for Looper {
    fn event_sender(&self) -> &dyn EventSender {
        &self.event_sender
    }
    fn backend(&self) -> &WGPUBackend {
        self.backend.as_ref().unwrap().as_ref()
    }

    fn new_event_sender(&self) -> Box<dyn EventSender> {
        Box::new(DetailEventSender {
            proxy: self.event_proxy.clone(),
        })
    }
}

impl Looper {
    pub fn new(builder: WindowBuilder) -> Self {
        let event_loop = EventLoop::with_user_event();
        let window = builder.build(&event_loop).unwrap();

        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(m) = window.current_monitor() {
                let msize: winit::dpi::LogicalSize<u32> = m.size().to_logical(m.scale_factor());
                let size: winit::dpi::LogicalSize<u32> =
                    window.outer_size().to_logical(m.scale_factor());
                if msize.width > size.width && msize.height > size.height {
                    let x = (msize.width - size.width) / 2;
                    let y = (msize.height - size.height) / 2;
                    window.set_outer_position(winit::dpi::LogicalPosition::new(x, y));
                }
            }
            window.set_ime_allowed(true);
        }

        let event_proxy = event_loop.create_proxy();
        Self {
            backend: None,
            event_loop: RefCell::new(event_loop.into()),
            first_render: false,
            event_sender: DetailEventSender {
                proxy: event_proxy.clone(),
            },
            event_proxy,
            processors: Vec::new(),
            frame: Statistics::new(Duration::from_millis(1000), Some(1.0 / 60.0)),
            inner: None,
            window: Arc::new(window),
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn bind(&mut self, backend: Box<WGPUBackend>, rm: Arc<ResourceManager>) {
        self.inner = Some(Rc::new(RefCell::new(LooperInner::new(
            backend.gpu(),
            self.window.clone(),
            rm,
        ))));
        self.backend = Some(backend);
        let p = self.inner.clone().unwrap();
        {
            let v = p.as_ref().borrow();

            self.processors.push(v.ui.event_processor().into());
        }
        self.processors
            .push(RefCell::new(Box::new(DefaultProcessor {
                inner: self.inner.as_ref().unwrap().clone(),
            })));
    }

    pub fn register_processor(&mut self, processor: Box<dyn EventProcessor>) {
        self.processors.push(processor.into());
    }

    fn run_event_processor(&self, event: &Event) -> ControlFlow {
        for process in &self.processors {
            match process.borrow_mut().on_event(self, event) {
                ProcessEventResult::Received => {
                    continue;
                }
                ProcessEventResult::Consumed => {
                    return ControlFlow::Wait;
                }
                ProcessEventResult::ExitLoop => {
                    return ControlFlow::Exit;
                }
            }
        }
        ControlFlow::Wait
    }

    pub fn run(&mut self) {
        let event_loop = self.event_loop.take().unwrap();
        let this = self as *mut Self;

        event_loop.run(move |ev, w, c| {
            let s = unsafe { this.as_mut().unwrap() };
            let event_proxy = s.event_proxy.clone();
            let control = s.on_event(ev, w, &event_proxy);
            *c = control;
        })
    }

    fn map_event(&self, event: WindowEvent) -> Option<Event> {
        Some(match event {
            WindowEvent::Resized(_) => {
                let size = self.window.inner_size();
                let logical: LogicalSize<u32> = size.to_logical(self.window.scale_factor());
                Event::Resized {
                    physical: Size::new(size.width, size.height),
                    logical: Size::new(logical.width, logical.height),
                }
            }
            WindowEvent::Moved(pos) => Event::Moved(Size::new(pos.x as u32, pos.y as u32)),
            WindowEvent::CloseRequested => Event::CloseRequested,
            WindowEvent::Focused(f) => Event::Focused(f),
            ev => Event::Input(match ev {
                WindowEvent::ReceivedCharacter(c) => InputEvent::ReceivedCharacter(c),
                WindowEvent::Ime(ime) => match ime {
                    winit::event::Ime::Commit(s) => InputEvent::ReceivedString(s),
                    _ => {
                        return None;
                    }
                },
                WindowEvent::KeyboardInput {
                    device_id,
                    input,
                    is_synthetic,
                } => InputEvent::KeyboardInput(core::event::KeyboardInput {
                    state: util::match_state(input.state),
                    vk: util::match_vk(input.virtual_keycode),
                }),
                WindowEvent::ModifiersChanged(state) => {
                    InputEvent::ModifiersChanged(core::event::ModifiersState {
                        ctrl: state.ctrl(),
                        win: state.logo(),
                        alt: state.alt(),
                        shift: state.shift(),
                    })
                }
                WindowEvent::CursorMoved {
                    device_id,
                    position,
                    modifiers: _,
                } => {
                    let logical: LogicalPosition<u32> =
                        position.to_logical(self.window.scale_factor());
                    InputEvent::CursorMoved {
                        physical: Vec2f::new(position.x as f32, position.y as f32),
                        logical: Vec2f::new(logical.x as f32, logical.y as f32),
                    }
                }
                WindowEvent::CursorEntered { device_id } => InputEvent::CursorEntered,
                WindowEvent::CursorLeft { device_id } => InputEvent::CursorLeft,
                WindowEvent::MouseWheel {
                    device_id,
                    delta,
                    phase,
                    modifiers: _,
                } => InputEvent::MouseWheel {
                    delta: match delta {
                        winit::event::MouseScrollDelta::LineDelta(x, y) => Vec3f::new(x, y, 0f32),
                        winit::event::MouseScrollDelta::PixelDelta(p) => {
                            Vec3f::new(p.x as f32 * 10f32, p.y as f32 * 10f32, 0f32)
                        }
                    },
                },
                WindowEvent::MouseInput {
                    device_id,
                    state,
                    button,
                    modifiers: _,
                } => {
                    if let Some(button) = util::match_button(button) {
                        InputEvent::MouseInput {
                            state: util::match_state(state),
                            button,
                        }
                    } else {
                        return None;
                    }
                }
                WindowEvent::ThemeChanged(theme) => {
                    return match theme {
                        winit::window::Theme::Light => Some(Event::Theme(Theme::Light)),
                        winit::window::Theme::Dark => Some(Event::Theme(Theme::Dark)),
                    }
                }
                WindowEvent::ScaleFactorChanged {
                    scale_factor,
                    new_inner_size,
                } => {
                    return Some(Event::ScaleFactorChanged(scale_factor));
                }
                _ => {
                    return None;
                }
            }),
        })
    }

    fn on_event(
        &mut self,
        original_event: WEvent,
        target: &EventLoopWindowTarget<Event>,
        event_proxy: &EventLoopProxy<Event>,
    ) -> ControlFlow {
        let mut ret = ControlFlow::Wait;
        match original_event {
            WEvent::WindowEvent { event, window_id } => {
                let ev = self.map_event(event);
                if let Some(ev) = ev {
                    ret = self.run_event_processor(&ev)
                }
            }
            WEvent::MainEventsCleared => {
                self.window.request_redraw();
            }
            WEvent::RedrawEventsCleared => {
                let (ins, d, ok) = self.frame.next_frame();
                if ok {
                    let _ = event_proxy.send_event(Event::Update(d.as_secs_f64()));
                }
            }
            WEvent::UserEvent(event) => match event {
                Event::CustomEvent(event) => match event {
                    CustomEvent::Exit => {
                        ret = ControlFlow::Exit;
                    }
                    CustomEvent::OpenUrl(url) => {
                        if let Err(err) = webbrowser::open(&url) {
                            log::error!("{}", err);
                        }
                    }
                    ev => {
                        ret = self.run_event_processor(&Event::CustomEvent(ev));
                    }
                },
                ev => {
                    let (ok, render) = match ev {
                        Event::Update(_) => (self.frame.new_frame(), false),
                        Event::Render => (true, true),
                        _ => (true, false),
                    };
                    if ok {
                        ret = self.run_event_processor(&ev);
                        if render && !self.first_render {
                            self.first_render = true;
                            self.window.set_visible(true);
                        }
                    }
                }
            },
            WEvent::NewEvents(cause) => match cause {
                winit::event::StartCause::ResumeTimeReached {
                    start,
                    requested_resume,
                } => {
                    self.window.request_redraw();
                    ret = ControlFlow::Poll;
                }
                winit::event::StartCause::Init => {
                    log::info!("init window scale {}", self.window.scale_factor());
                    ret = self
                        .run_event_processor(&Event::ScaleFactorChanged(self.window.scale_factor()))
                }
                _ => {}
            },
            _ => {}
        };
        if ret == ControlFlow::Wait {
            let (ins, _, _) = self.frame.next_frame();
            ret = ControlFlow::WaitUntil(ins);
        }
        if self.frame.changed() {
            self.window
                .set_title(&format!("GStudy {:.1}fps", self.frame.fps()));
        }
        ret
    }
}
