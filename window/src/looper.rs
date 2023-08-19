use core::{
    backends::wgpu_backend::WGPUResource,
    context::RContextRef,
    event::{EventProcessor, EventRegistry, EventSender, EventSource, ProcessEventResult},
};
use std::{
    any::{Any, TypeId},
    cell::RefCell,
    sync::Arc,
};

use instant::Duration;
use winit::{
    event::StartCause,
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget},
    window::WindowBuilder,
};

use crate::{statistics::Statistics, window::Window, CEvent, DEvent, Event, WEvent};

pub struct LooperEventSource {
    event_proxy: EventLoopProxy<DEvent>,
}

pub struct Looper {
    event_loop: RefCell<Option<EventLoop<DEvent>>>,
    main_window: Option<RefCell<Window>>,

    event_proxy: EventLoopProxy<DEvent>,
    frame: Statistics,
    event_registry: LoopEventRegistry,

    auto_exit: bool,
    has_render_event: bool,
}

impl EventSender for LooperEventSource {
    fn send_event(&self, ev: Box<dyn Any + Send>) {
        self.event_proxy.send_event(ev).unwrap();
    }
}

impl EventSource for LooperEventSource {
    fn event_sender(&self) -> &dyn EventSender {
        self
    }

    fn new_event_sender(&self) -> Box<dyn EventSender> {
        Box::new(Self {
            event_proxy: self.event_proxy.clone(),
        })
    }
}

#[derive(Default)]
pub struct LoopEventRegistry {
    processors: Vec<RefCell<Box<dyn EventProcessor>>>,
}

impl LoopEventRegistry {
    fn run_event_processor(&mut self, event: &dyn Any, s: &LooperEventSource) -> ControlFlow {
        for process in &self.processors {
            match process.borrow_mut().on_event(s, event) {
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
}

impl EventRegistry for LoopEventRegistry {
    fn register_processor(&mut self, processor: Box<dyn EventProcessor>) {
        self.processors.push(processor.into());
    }
}

impl Looper {
    pub fn new() -> Self {
        let event_loop = EventLoopBuilder::with_user_event().build();
        let event_proxy = event_loop.create_proxy();
        Self {
            event_loop: RefCell::new(Some(event_loop)),
            main_window: None,

            event_proxy,
            frame: Statistics::new(Duration::from_millis(1000), Some(1.0 / 60.0)),
            event_registry: LoopEventRegistry::default(),
            auto_exit: true,
            has_render_event: false,
        }
    }

    pub fn event_registry(&mut self) -> &mut dyn EventRegistry {
        &mut self.event_registry
    }

    pub fn event_source(&self) -> LooperEventSource {
        LooperEventSource {
            event_proxy: self.event_proxy.clone(),
        }
    }

    pub fn create_window(&mut self, b: WindowBuilder, context: RContextRef) -> Arc<WGPUResource> {
        let ev = self.event_loop.borrow();
        let ev = ev.as_ref().unwrap();
        let w = b.build(&ev).unwrap();

        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(m) = w.current_monitor() {
                log::info!(
                    "monitor {} size {:?}  fac {}",
                    m.name().unwrap_or_default(),
                    m.size(),
                    m.scale_factor()
                );
                let msize: winit::dpi::LogicalSize<u32> = m.size().to_logical(m.scale_factor());
                let size: winit::dpi::LogicalSize<u32> =
                    w.outer_size().to_logical(m.scale_factor());
                if msize.width > size.width && msize.height > size.height {
                    let x = (msize.width - size.width) / 2;
                    let y = (msize.height - size.height) / 2;
                    log::info!("window relocated {},{}", x, y);
                    w.set_outer_position(winit::dpi::LogicalPosition::new(x, y));
                }
            }
            w.set_ime_allowed(true);
        }

        if self.main_window.is_none() {
            self.main_window = Some(RefCell::new(Window::new(
                w,
                context,
                &mut self.event_registry,
            )));
            self.main_window.as_ref().unwrap().borrow().gpu()
        } else {
            panic!("main window exists");
            // self.ext_window.push(Window::new(w));
        }
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

    fn process(&mut self, event: &dyn Any) -> ControlFlow {
        if let Some(c) = event.downcast_ref::<CEvent>() {
            match c {
                core::event::Event::PreUpdate(delta) => {
                    self.has_render_event = true;
                    log::debug!("pre update");
                    self.frame.new_frame();
                    let _ = self
                        .event_proxy
                        .send_event(Box::new(CEvent::Update(*delta)));
                }
                core::event::Event::Update(delta) => {
                    log::debug!("update");
                    let _ = self
                        .event_proxy
                        .send_event(Box::new(CEvent::PostUpdate(*delta)));
                }
                core::event::Event::PostRender => {
                    self.has_render_event = false;
                }
                _ => (),
            }
        } else if let Some(c) = event.downcast_ref::<Event>() {
            if let Event::Exit = c {
                return ControlFlow::Exit;
            }
            if self.auto_exit {
                if let Event::CloseRequested = c {
                    let _ = self.event_proxy.send_event(Box::new(Event::Exit));
                }
            }
        }

        let mut w = self.main_window.as_mut().unwrap().borrow_mut();
        w.on_event(
            &LooperEventSource {
                event_proxy: self.event_proxy.clone(),
            },
            event,
        );

        self.event_registry.run_event_processor(
            event,
            &LooperEventSource {
                event_proxy: self.event_proxy.clone(),
            },
        )
    }

    fn on_event(
        &mut self,
        original_event: WEvent,
        target: &EventLoopWindowTarget<DEvent>,
        event_proxy: &EventLoopProxy<DEvent>,
    ) -> ControlFlow {
        let mut ret = ControlFlow::Wait;

        match &original_event {
            WEvent::WindowEvent {
                event: _,
                window_id: _,
            } => {
                let mut w = self.main_window.as_mut().unwrap().borrow_mut();
                let ev = w.on_translate_event(original_event, event_proxy);
                if let Some(ev) = ev {
                    drop(w);
                    ret = self.process(ev.as_ref())
                }
            }
            WEvent::MainEventsCleared => {
                #[cfg(windows)]
                {
                    self.window.request_redraw();
                }
            }
            WEvent::RedrawEventsCleared => {
                let (_, d, ok) = self.frame.next_frame();
                if ok {
                    if !self.has_render_event {
                        let to_event: Box<dyn Any + Send> =
                            Box::new(CEvent::PreUpdate(d.as_secs_f64()));
                        let _ = event_proxy.send_event(to_event);
                        ret = ControlFlow::Poll;
                    }
                }
            }
            WEvent::UserEvent(event) => {
                ret = self.process(event.as_ref());
            }
            WEvent::NewEvents(c) => {
                let mut w = self.main_window.as_mut().unwrap().borrow_mut();
                let ev = w.on_translate_event(original_event, event_proxy);
                if let Some(ev) = ev {
                    drop(w);
                    ret = self.process(ev.as_ref())
                }
            }
            _ => {}
        };

        match ret {
            ControlFlow::Wait => {
                let (ins, _, _) = self.frame.next_frame();
                ret = ControlFlow::WaitUntil(ins);
            }
            ControlFlow::ExitWithCode(_) => {
                log::warn!("app exit");
            }
            _ => {}
        }
        ret
    }
}
