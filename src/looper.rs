use std::cell::RefCell;
use std::time::Duration;

use crate::backends::wgpu_backend::WGPUBackend;
use crate::event::*;
use crate::statistics::Statistics;
use crate::types::*;
use winit::event::WindowEvent;
use winit::event_loop::{ControlFlow, EventLoop, EventLoopProxy, EventLoopWindowTarget};
use winit::window::Window;
use winit::window::{self, WindowBuilder};
type WEvent<'a> = winit::event::Event<'a, Event>;

pub struct Looper {
    window: Window,
    backend: Option<WGPUBackend>,
    event_loop: RefCell<Option<EventLoop<Event>>>,
    event_proxy: EventLoopProxy<Event>,
    processors: Vec<RefCell<Box<dyn EventProcessor>>>,
    frame: Statistics,
    first_render: bool,
}

pub struct DefaultProcessor {}

impl DefaultProcessor {
    pub fn new() -> Self {
        Self {}
    }
}

impl EventProcessor for DefaultProcessor {
    fn on_event(
        &mut self,
        source: &dyn EventSource,
        event: &crate::event::Event,
    ) -> ProcessEventResult {
        match event {
            Event::CloseRequested => {
                return ProcessEventResult::ExitLoop;
            }
            Event::UpdateCursor(c) => {
                source.window().set_cursor_icon(*c);
            }
            Event::UpdateImePosition(pos) => {
                source
                    .window()
                    .set_ime_position(winit::dpi::Position::Logical(
                        winit::dpi::LogicalPosition::new(pos.0 as f64, pos.1 as f64),
                    ));
            }
            Event::FullScreen(fullscreen) => {
                if *fullscreen {
                    source
                        .window()
                        .set_fullscreen(Some(window::Fullscreen::Borderless(None)));
                } else {
                    source.window().set_fullscreen(None);
                }
            }
            _ => (),
        };
        ProcessEventResult::Received
    }
}

impl EventSource for Looper {
    fn window(&self) -> &winit::window::Window {
        &self.window
    }

    fn event_proxy(&self) -> winit::event_loop::EventLoopProxy<Event> {
        self.event_proxy.clone()
    }
    fn backend(&self) -> &WGPUBackend {
        self.backend.as_ref().unwrap()
    }
}

impl Looper {
    pub fn new(builder: WindowBuilder) -> Self {
        let event_loop = EventLoop::with_user_event();
        let window = builder.build(&event_loop).unwrap();
        if let Some(m) = window.current_monitor() {
            let msize: winit::dpi::LogicalSize<u32> = m.size().to_logical(m.scale_factor());
            let size: winit::dpi::LogicalSize<u32> =
                window.outer_size().to_logical(m.scale_factor());
            let x = (msize.width - size.width) / 2;
            let y = (msize.height - size.height) / 2;
            window.set_outer_position(winit::dpi::LogicalPosition::new(x, y));
        }
        window.set_ime_allowed(true);

        let event_proxy = event_loop.create_proxy();
        Self {
            window,
            backend: None,
            event_loop: RefCell::new(event_loop.into()),
            first_render: false,
            event_proxy,
            processors: Vec::new(),
            frame: Statistics::new(Duration::from_millis(1000), Some(1.0 / 60.0)),
        }
    }

    pub fn bind_backend(&mut self, backend: WGPUBackend) {
        self.backend = Some(backend)
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
            WindowEvent::Resized(size) => Event::Resized(Size::new(size.width, size.height)),
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
                } => InputEvent::KeyboardInput {
                    device_id,
                    input,
                    is_synthetic,
                },
                WindowEvent::ModifiersChanged(state) => InputEvent::ModifiersChanged(state),
                WindowEvent::CursorMoved {
                    device_id,
                    position,
                    modifiers: _,
                } => InputEvent::CursorMoved {
                    device_id,
                    position,
                },
                WindowEvent::CursorEntered { device_id } => InputEvent::CursorEntered { device_id },
                WindowEvent::CursorLeft { device_id } => InputEvent::CursorLeft { device_id },
                WindowEvent::MouseWheel {
                    device_id,
                    delta,
                    phase,
                    modifiers: _,
                } => InputEvent::MouseWheel {
                    device_id,
                    delta,
                    phase,
                },
                WindowEvent::MouseInput {
                    device_id,
                    state,
                    button,
                    modifiers: _,
                } => InputEvent::MouseInput {
                    device_id,
                    state,
                    button,
                },
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
