use std::{
    collections::{HashMap, VecDeque},
    mem::swap,
    sync::Arc,
    thread::spawn,
    time::{Duration, Instant},
};

use parking_lot::{Condvar, Mutex, RwLock};
use winit::{
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize},
    event::{
        DeviceEvent, DeviceId, ElementState, Event, KeyboardInput, ModifiersState, MouseButton,
        MouseScrollDelta, Touch, TouchPhase, WindowEvent,
    },
    event_loop::{ControlFlow, EventLoop, EventLoopProxy, EventLoopWindowTarget},
    platform::run_return::EventLoopExtRunReturn,
    window::{Fullscreen, Window, WindowBuilder, WindowId},
};

use crate::{
    gpu_context::{GpuAttachResource, GpuContext, GpuContextRef},
    statistics::Statistics,
    types::{Color, Quaternion, Size},
    ui::{logic::UILogicRef, UIRenderer},
};
use futures::executor::block_on;

use winit::dpi::Position;
#[derive(Debug)]
pub enum InnerEvent {
    PostClose(WindowId),
}

#[derive(Debug)]
pub struct NewWindowProps {
    pub title: String,
    pub size: Size,
    pub pos: Size,
    pub drag: bool,
    pub from_window_id: WindowId,
    pub logic_window_id: u64,
}

#[derive(Debug)]
pub struct CloseWindowProps {
    pos: Size,
    drag: bool,
    count: u32,
}

#[derive(Debug)]
pub enum GlobalUserEvent {
    Copy(String),
    OpenUrl(String),
    Exit,

    CanvasResize(Size),
    ModuleChanged(&'static str),

    NewWindow(NewWindowProps),
    CloseWindow(CloseWindowProps),
}

#[derive(Debug)]
pub enum UserEvent {
    Global(GlobalUserEvent),
    Window(WindowId, WindowUserEvent),
    Inner(InnerEvent),
}

#[derive(Debug)]
pub enum WindowUserEvent {
    UpdateCursor(egui::CursorIcon),
    UpdateIme(Position),
    FullScreen(bool),
    FrameRate(Option<u32>),
    ShowWindow(bool),
    PostNewWindow(MakeWindowResult),
    // ClearColor(Option<Color>),
}

#[derive(Debug)]
pub enum RenderWindowInputEvent {
    KeyboardInput {
        device_id: DeviceId,
        input: KeyboardInput,
        is_synthetic: bool,
    },
    ModifiersChanged(ModifiersState),

    CursorMoved {
        device_id: DeviceId,
        position: PhysicalPosition<f64>,
    },

    ReceivedCharacter(char),

    CursorEntered {
        device_id: DeviceId,
    },

    CursorLeft {
        device_id: DeviceId,
    },

    MouseWheel {
        device_id: DeviceId,
        delta: MouseScrollDelta,
        phase: TouchPhase,
    },

    MouseInput {
        device_id: DeviceId,
        state: ElementState,
        button: MouseButton,
    },
}

#[derive(Debug)]
pub enum RenderWindowEvent {
    // need update window
    Update,

    UserEvent(WindowUserEvent),

    // raw input event
    RawInput(DeviceEvent),

    Input(RenderWindowInputEvent),

    Resized(Size),

    Moved(Size),

    CloseRequested,

    Focused(bool),
}

#[derive(Debug)]
pub struct Queue {
    vec: Mutex<VecDeque<RenderWindowEvent>>,
    var: Condvar,
}

impl Queue {
    pub fn new() -> Self {
        Self {
            vec: Mutex::new(VecDeque::with_capacity(1024)),
            var: Condvar::new(),
        }
    }
    pub fn fetch(&self, timeout: Instant) -> Option<RenderWindowEvent> {
        let mut guard = self.vec.lock();
        loop {
            let ev = guard.pop_front();
            match ev {
                Some(v) => break Some(v),
                None => {
                    if self.var.wait_until(&mut guard, timeout).timed_out() {
                        break None;
                    }
                }
            }
        }
    }
    pub fn send(&self, event: RenderWindowEvent) {
        {
            let mut guard = self.vec.lock();
            guard.push_back(event);
        }
        self.var.notify_one();
    }
}

pub struct RenderWindowEventLoop {
    map: HashMap<WindowId, (Window, Arc<Queue>)>,
    gpu_context: GpuContextRef,
}

impl RenderWindowEventLoop {
    pub fn new(gpu_context: GpuContextRef) -> Self {
        Self {
            map: HashMap::new(),
            gpu_context,
        }
    }

    pub fn add_render_window(&mut self, window: Window, queue: Arc<Queue>) {
        self.map.insert(window.id(), (window, queue));
    }

    pub fn remove_render_window(&mut self, window: WindowId) {
        self.map.remove(&window);
    }

    pub fn run<F>(&mut self, f: F)
    where
        F: FnOnce(
            &mut RenderWindowEventLoop,
            &EventLoopProxy<UserEvent>,
            &EventLoopWindowTarget<UserEvent>,
        ),
    {
        let mut event_loop = EventLoop::with_user_event();
        let event_proxy = event_loop.create_proxy();

        f(self, &event_proxy, &event_loop);

        event_loop.run_return(|event, target, control_flow| {
            let ret = self.event_loop(event, target, &event_proxy);
            *control_flow = ret;
        });
    }

    fn map_event(&self, event: WindowEvent) -> Option<RenderWindowEvent> {
        Some(match event {
            WindowEvent::Resized(size) => {
                RenderWindowEvent::Resized(Size::new(size.width, size.height))
            }
            WindowEvent::Moved(pos) => {
                RenderWindowEvent::Moved(Size::new(pos.x as u32, pos.y as u32))
            }
            WindowEvent::CloseRequested => RenderWindowEvent::CloseRequested,
            WindowEvent::Focused(f) => RenderWindowEvent::Focused(f),
            ev => RenderWindowEvent::Input(match ev {
                WindowEvent::ReceivedCharacter(c) => RenderWindowInputEvent::ReceivedCharacter(c),
                WindowEvent::KeyboardInput {
                    device_id,
                    input,
                    is_synthetic,
                } => RenderWindowInputEvent::KeyboardInput {
                    device_id,
                    input,
                    is_synthetic,
                },
                WindowEvent::ModifiersChanged(state) => {
                    RenderWindowInputEvent::ModifiersChanged(state)
                }
                WindowEvent::CursorMoved {
                    device_id,
                    position,
                    modifiers: _,
                } => RenderWindowInputEvent::CursorMoved {
                    device_id,
                    position,
                },
                WindowEvent::CursorEntered { device_id } => {
                    RenderWindowInputEvent::CursorEntered { device_id }
                }
                WindowEvent::CursorLeft { device_id } => {
                    RenderWindowInputEvent::CursorLeft { device_id }
                }
                WindowEvent::MouseWheel {
                    device_id,
                    delta,
                    phase,
                    modifiers: _,
                } => RenderWindowInputEvent::MouseWheel {
                    device_id,
                    delta,
                    phase,
                },
                WindowEvent::MouseInput {
                    device_id,
                    state,
                    button,
                    modifiers: _,
                } => RenderWindowInputEvent::MouseInput {
                    device_id,
                    state,
                    button,
                },
                _ => {
                    return None;
                }
            }),
        })
    }

    fn event_loop(
        &mut self,
        original_event: Event<UserEvent>,
        target: &EventLoopWindowTarget<UserEvent>,
        event_proxy: &EventLoopProxy<UserEvent>,
    ) -> ControlFlow {
        let mut ret = ControlFlow::Wait;
        match original_event {
            Event::WindowEvent { event, window_id } => {
                let render_window = self.map.get(&window_id);
                if let Some((_, queue)) = render_window {
                    if let Some(ev) = self.map_event(event) {
                        queue.send(ev);
                    }
                }
            }
            Event::RedrawRequested(window_id) => {
                let render_window = self.map.get(&window_id);
                if let Some((_, queue)) = render_window {
                    queue.send(RenderWindowEvent::Update);
                }
            }
            Event::UserEvent(event) => match event {
                UserEvent::Global(event) => match event {
                    GlobalUserEvent::Exit => {
                        ret = ControlFlow::Exit;
                    }
                    GlobalUserEvent::Copy(str) => {
                        tinyfiledialogs::message_box_ok(
                            "copy",
                            &str,
                            tinyfiledialogs::MessageBoxIcon::Info,
                        );
                    }
                    GlobalUserEvent::OpenUrl(str) => {
                        tinyfiledialogs::message_box_ok(
                            "open url",
                            &str,
                            tinyfiledialogs::MessageBoxIcon::Info,
                        );
                    }
                    GlobalUserEvent::NewWindow(props) => {
                        let render_window = self.map.get(&props.from_window_id);
                        let mut new_pos = props.pos;
                        if let Some((window, queue)) = render_window {
                            let offset = window.outer_position().unwrap();
                            let mut offset = Size::new(offset.x as u32, offset.y as u32);
                            new_pos.x += offset.x as u32;
                            new_pos.y += offset.y as u32;
                        }
                        let (new_window, resource) = RenderWindow::make_window(
                            props.title,
                            new_pos,
                            props.size,
                            target,
                            &self.gpu_context,
                        );
                        let id = new_window.id();
                        let new_queue = Arc::new(Queue::new());
                        self.add_render_window(new_window, new_queue.clone());
                        let t = MakeWindowResult {
                            queue: new_queue,
                            resource,
                            window_id: id,
                            size: props.size,
                            logic_window_id: props.logic_window_id,
                        };
                        let render_window = self.map.get(&props.from_window_id);
                        if let Some((_, queue)) = render_window {
                            queue.send(RenderWindowEvent::UserEvent(
                                WindowUserEvent::PostNewWindow(t),
                            ));
                        }
                    }
                    _ => (),
                },
                UserEvent::Window(window_id, event) => {
                    let render_window = self.map.get(&window_id);
                    if let Some((window, queue)) = render_window {
                        if !RenderWindow::do_window_event(window, &event) {
                            queue.send(RenderWindowEvent::UserEvent(event));
                        }
                    }
                }
                UserEvent::Inner(event) => match event {
                    InnerEvent::PostClose(window_id) => {
                        self.map.remove(&window_id);
                        if self.map.len() == 0 {
                            ret = ControlFlow::Exit;
                        }
                    }
                },
            },
            _ => (),
        };
        ret
    }
}

pub struct RenderWindow {
    render_tick: Instant,
    statistics: Statistics,

    gpu_context: GpuContextRef,
    event_proxy: EventLoopProxy<UserEvent>,

    closed: bool,
    queue: Arc<Queue>,
    id: WindowId,
}

#[derive(Debug)]
pub struct MakeWindowResult {
    queue: Arc<Queue>,
    resource: GpuAttachResource,
    window_id: WindowId,
    logic_window_id: u64,
    size: Size,
}

impl RenderWindow {
    pub fn make_window(
        title: String,
        pos: Size,
        size: Size,
        window_target: &EventLoopWindowTarget<UserEvent>,
        gpu_context: &GpuContext,
    ) -> (Window, GpuAttachResource) {
        let window = WindowBuilder::new()
            .with_inner_size(LogicalSize::new(size.x as u32, size.y as u32))
            .with_position(LogicalPosition::new(pos.x as i32, pos.y as i32))
            .with_title(title)
            .with_visible(false)
            .build(window_target)
            .unwrap();

        if pos.x as i32 == 0 && pos.y as i32 == 0 {
            if let Some(m) = window.current_monitor() {
                let mut mpos = m.position();
                let msize = m.size();
                mpos.x += (msize.width / 2) as i32 - size.x as i32 / 2;
                mpos.y += (msize.height / 2) as i32 - size.y as i32 / 2;
                window.set_outer_position(mpos);
            }
        }

        let resource = gpu_context.attach_window(&window);
        (window, resource)
    }

    pub fn new(
        gpu_context: GpuContextRef,
        queue: Arc<Queue>,
        id: WindowId,
        event_proxy: EventLoopProxy<UserEvent>,
    ) -> Self {
        Self {
            render_tick: Instant::now(),
            statistics: Statistics::new(Duration::from_millis(900), None),
            gpu_context,
            closed: false,
            queue,
            id,
            event_proxy,
        }
    }

    pub fn dispatch_window(
        mut self,
        logic_window_id: u64,
        resource: GpuAttachResource,
        ui_logic: UILogicRef,
        size: Size,
    ) {
        spawn(move || {
            self.thread_main(logic_window_id, resource, ui_logic, size);
        });
    }

    pub fn id(&self) -> WindowId {
        self.id
    }

    pub fn closed(&self) -> bool {
        self.closed
    }

    pub fn set_frame_lock(&mut self, target_frame_seconds: Option<f32>) {
        self.statistics.set_frame_lock(target_frame_seconds);
    }

    fn thread_main(
        mut self,
        logic_window_id: u64,
        resource: GpuAttachResource,
        ui_logic: UILogicRef,
        size: Size,
    ) {
        block_on(self.gpu_context.attach_resource(resource));
        let mut ui_renderer = UIRenderer::new(
            self.gpu_context.clone(),
            size,
            self.event_proxy.clone(),
            ui_logic,
        );
        self.event_proxy
            .send_event(UserEvent::Window(
                self.id,
                WindowUserEvent::ShowWindow(true),
            ))
            .ok();
        ui_renderer.rebind_window(logic_window_id);
        loop {
            let event = self.queue.fetch(self.render_tick);
            match event {
                Some(event) => {
                    self.do_event(event, &mut ui_renderer);
                }
                None => {
                    self.do_event(RenderWindowEvent::Update, &mut ui_renderer);
                }
            }
        }
    }

    fn do_window_event(window: &Window, event: &WindowUserEvent) -> bool {
        match event {
            WindowUserEvent::UpdateCursor(cursor) => {
                match crate::util::match_winit_cursor(*cursor) {
                    Some(c) => {
                        window.set_cursor_visible(true);
                        window.set_cursor_icon(c);
                    }
                    None => {
                        window.set_cursor_visible(false);
                    }
                };
                return false;
            }
            WindowUserEvent::UpdateIme(pos) => {
                window.set_ime_position(*pos);
            }
            WindowUserEvent::FullScreen(set) => {
                if *set {
                    window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                } else {
                    window.set_fullscreen(None);
                }
            }
            WindowUserEvent::ShowWindow(show) => {
                window.set_visible(*show);
            }
            _ => return false,
        }
        true
    }

    fn do_event(&mut self, event: RenderWindowEvent, ui_renderer: &mut UIRenderer) {
        ui_renderer.on_event(&event);
        match event {
            RenderWindowEvent::Input(event) => {}
            RenderWindowEvent::CloseRequested => {
                log::info!("close window");
                self.closed = true;
                self.gpu_context.detach();
                self.event_proxy
                    .send_event(UserEvent::Inner(InnerEvent::PostClose(self.id)))
                    .ok();
            }
            RenderWindowEvent::Update => {
                if self.closed {
                    return;
                }
                let now = Instant::now();
                if now < self.render_tick {
                    // self.window.request_redraw();
                    return;
                }
                self.statistics.new_frame();
                let need_draw = ui_renderer.update(&self.statistics);
                if need_draw {
                    ui_renderer.render();
                }
                self.render_tick = self.statistics.get_waiting();
            }
            RenderWindowEvent::UserEvent(e) => match e {
                WindowUserEvent::FrameRate(rate) => {
                    log::info!("frame rate set {:?}", rate);
                    self.set_frame_lock(rate.map(|v| 1f32 / v as f32));
                }
                WindowUserEvent::PostNewWindow(t) => Self::new(
                    self.gpu_context.clone(),
                    t.queue,
                    t.window_id,
                    self.event_proxy.clone(),
                )
                .dispatch_window(
                    t.logic_window_id,
                    t.resource,
                    ui_renderer.logic(),
                    t.size,
                ),
                _ => (),
            },
            _ => (),
        }
    }
}
