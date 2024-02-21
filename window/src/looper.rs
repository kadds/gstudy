use core::{
    backends::wgpu_backend::WGPUResource,
    context::RContextRef,
    event::{EventProcessor, EventRegistry, EventSender, EventSource, ProcessEventResult},
};
use std::{
    any::Any,
    cell::RefCell,
    sync::{Arc, Mutex},
};

use instant::Duration;
use log::{error, warn};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::{
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
    frame: Arc<Mutex<Statistics>>,
    event_registry: LoopEventRegistry,

    auto_exit: bool,
    has_render_event: bool,
    is_first_update: bool,
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
    fn run_event_processor(
        &mut self,
        event: &dyn Any,
        s: &LooperEventSource,
    ) -> Option<ControlFlow> {
        for process in &self.processors {
            match process.borrow_mut().on_event(s, event) {
                ProcessEventResult::Received => {
                    continue;
                }
                ProcessEventResult::Consumed => {
                    return Some(ControlFlow::Wait);
                }
                ProcessEventResult::ExitLoop => {
                    return None;
                }
            }
        }
        Some(ControlFlow::Wait)
    }
}

impl EventRegistry for LoopEventRegistry {
    fn register_processor(&mut self, processor: Box<dyn EventProcessor>) {
        self.processors.push(processor.into());
    }
}

impl Looper {
    pub fn new() -> Self {
        let event_loop = EventLoopBuilder::with_user_event().build().unwrap();
        let event_proxy = event_loop.create_proxy();
        Self {
            event_loop: RefCell::new(Some(event_loop)),
            main_window: None,

            event_proxy,
            frame: Arc::new(Mutex::new(Statistics::new(
                Duration::from_millis(1000),
                Some(1.0 / 60.0),
            ))),
            event_registry: LoopEventRegistry::default(),
            auto_exit: true,
            has_render_event: false,
            is_first_update: true,
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

    pub fn statistics(&self) -> Arc<Mutex<Statistics>> {
        self.frame.clone()
    }

    pub fn handle(&self) -> Option<RawWindowHandle> {
        self.main_window
            .as_ref()
            .map(|v| v.borrow().inner.window_handle().unwrap().as_raw())
    }

    pub fn create_window(&mut self, b: WindowBuilder, context: RContextRef) -> Arc<WGPUResource> {
        let ev = self.event_loop.borrow();
        let ev = ev.as_ref().unwrap();
        let w = b.build(ev).unwrap();

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

        let err = event_loop.run(move |ev, w| {
            let s = unsafe { this.as_mut().unwrap() };
            let event_proxy = s.event_proxy.clone();
            s.on_event(ev, w, &event_proxy);
        });
        if let Err(err) = err {
            error!("{}", err)
        }
    }

    fn process_inner(&mut self, event: &dyn Any) -> Option<ControlFlow> {
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

    fn process(&mut self, event: &dyn Any) -> Option<ControlFlow> {
        if let Some(c) = event.downcast_ref::<CEvent>() {
            match c {
                core::event::Event::PreUpdate(delta) => {
                    profiling::finish_frame!();
                    self.has_render_event = true;
                    log::debug!("pre update event");
                    profiling::scope!("pre update");
                    self.frame.lock().unwrap().new_frame();
                    let _ = self
                        .event_proxy
                        .send_event(Box::new(CEvent::Update(*delta)));
                    return self.process_inner(event);
                }
                core::event::Event::Update(delta) => {
                    log::debug!("update");
                    profiling::scope!("update event");
                    let _ = self
                        .event_proxy
                        .send_event(Box::new(CEvent::PostUpdate(*delta)));
                    return self.process_inner(event);
                }
                core::event::Event::PostUpdate(_) => {
                    self.has_render_event = false;
                    profiling::scope!("post update event");
                    return self.process_inner(event);
                }
                core::event::Event::PreRender => {
                    profiling::scope!("pre render event");
                    return self.process_inner(event);
                }
                core::event::Event::Render(_) => {
                    profiling::scope!("render event");
                    return self.process_inner(event);
                }
                core::event::Event::PostRender => {
                    profiling::scope!("post render event");
                    if let Some(w) = &self.main_window {
                        let ms = w.borrow().delay_frame_ms;
                        self.frame.lock().unwrap().set_delay_frame(ms);
                    }
                    return self.process_inner(event);
                }
                _ => (),
            }
        } else if let Some(c) = event.downcast_ref::<Event>() {
            if let Event::Exit = c {
                return None;
            }
            if self.auto_exit {
                if let Event::CloseRequested = c {
                    let _ = self.event_proxy.send_event(Box::new(Event::Exit));
                }
            }
        }

        self.process_inner(event)
    }

    fn on_event(
        &mut self,
        original_event: WEvent,
        target: &EventLoopWindowTarget<DEvent>,
        event_proxy: &EventLoopProxy<DEvent>,
    ) {
        let mut ret = Some(ControlFlow::Wait);

        match &original_event {
            WEvent::WindowEvent {
                event,
                window_id: _,
            } => {
                match event {
                    winit::event::WindowEvent::RedrawRequested => {
                        let (_, d, ok) = self.frame.lock().unwrap().next_frame();
                        if ok && !self.has_render_event {
                            if self.is_first_update {
                                self.is_first_update = false;
                                let _ = event_proxy.send_event(Box::new(CEvent::FirstSync));
                            } else {
                                let to_event: Box<dyn Any + Send> =
                                    Box::new(CEvent::PreUpdate(d.as_secs_f64()));
                                let _ = event_proxy.send_event(to_event);
                                ret = Some(ControlFlow::Poll);
                            }
                        }
                    }
                    _ => (),
                }
                let mut w = self.main_window.as_mut().unwrap().borrow_mut();
                let ev = w.on_translate_event(original_event, event_proxy);
                if let Some(ev) = ev {
                    drop(w);
                    ret = self.process(ev.as_ref())
                }
            }
            WEvent::AboutToWait => {
                #[cfg(windows)]
                {
                    let w = self.main_window.as_mut().unwrap().borrow_mut();
                    w.inner.request_redraw();
                }
            }
            WEvent::UserEvent(event) => {
                ret = self.process(event.as_ref());
            }
            WEvent::NewEvents(_c) => {
                let mut w = self.main_window.as_mut().unwrap().borrow_mut();
                let ev = w.on_translate_event(original_event, event_proxy);
                if let Some(ev) = ev {
                    drop(w);
                    ret = self.process(ev.as_ref())
                }
            }
            _ => {}
        };
        if let Some(ret) = ret {
            target.set_control_flow(ret);
        } else {
            target.exit();
        }
    }
}
