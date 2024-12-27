use core::{
    backends::wgpu_backend::WGPUResource,
    context::{RContext, RContextRef},
    event::{EventProcessor, EventRegistry, EventSender, EventSource, EventSourceInformation, ProcessEventResult},
};
use std::{
    any::Any,
    cell::RefCell,
    sync::{Arc, Mutex},
};

use instant::Duration;
use log::error;
use raw_window_handle::{HasRawWindowHandle, HasWindowHandle, RawWindowHandle};
use winit::{
    application::ApplicationHandler,
    event::StartCause,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy},
    window::WindowAttributes,
};

use crate::{statistics::Statistics, window::Window, CEvent, DEvent, Event, WEvent};

#[derive(Debug, Clone)]
pub struct LooperEventSource {
    event_proxy: EventLoopProxy<DEvent>,
    info: Option<EventSourceInformation>,
}

pub struct Looper {
    event_loop: RefCell<Option<EventLoop<DEvent>>>,
    main_window: Option<RefCell<Window>>,

    frame: Arc<Mutex<Statistics>>,
    event_registry: LoopEventRegistry,

    auto_exit: bool,
    has_render_event: bool,
    is_first_update: bool,

    default_window_attr: Option<WindowAttributes>,
    ctx: Arc<RContext>,
    src: LooperEventSource,
}

impl EventSender for LooperEventSource {
    fn send_event(&self, ev: Box<dyn Any + Send>) {
        self.event_proxy.send_event(Some(ev)).unwrap();
    }
}

impl EventSource for LooperEventSource {
    fn event_sender(&self) -> &dyn EventSender {
        self
    }

    fn new_event_sender(&self) -> Box<dyn EventSender> {
        Box::new(Self {
            event_proxy: self.event_proxy.clone(),
            info: self.info.clone() 
        })
    }

    fn source_information(&self) -> EventSourceInformation {
       self.info.as_ref().unwrap().clone()
    }
}

#[derive(Default)]
pub struct LoopEventRegistry {
    processors: Vec<RefCell<Box<dyn EventProcessor>>>,
}

impl LoopEventRegistry {
    fn run_event_processor(
        &self,
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

    fn run_init(
        &mut self,
        s: &LooperEventSource,
    ) -> Option<ControlFlow> {
        for process in &self.processors {
            process.borrow_mut().init(s);
        }
        Some(ControlFlow::Poll)
    }
}

impl EventRegistry for LoopEventRegistry {
    fn register_processor(&mut self, processor: Box<dyn EventProcessor>) {
        self.processors.push(processor.into());
    }
}

impl Looper {
    pub fn new(default_window: WindowAttributes, ctx: Arc<RContext>) -> Self {
        let event_loop = EventLoopBuilder::default().build().unwrap();
        let event_proxy = event_loop.create_proxy();
        Self {
            event_loop: RefCell::new(Some(event_loop)),
            main_window: None,

            frame: Arc::new(Mutex::new(Statistics::new(
                Duration::from_millis(1000),
                Some(1.0 / 60.0),
            ))),
            event_registry: LoopEventRegistry::default(),
            auto_exit: true,
            has_render_event: false,
            is_first_update: true,
            default_window_attr: Some(default_window),
            ctx,
            src: LooperEventSource {
                event_proxy,
                info: None,
            }
        }
    }

    fn gpu(&self) -> Arc<WGPUResource> {
        self.main_window.as_ref().unwrap().borrow().gpu()
    }

    pub fn event_registry(&mut self) -> &mut dyn EventRegistry {
        &mut self.event_registry
    }

    pub fn event_source(&self) -> LooperEventSource {
        self.src.clone()
    }

    pub fn statistics(&self) -> Arc<Mutex<Statistics>> {
        self.frame.clone()
    }

    pub fn handle(&self) -> Option<RawWindowHandle> {
        self.main_window
            .as_ref()
            .map(|v| v.borrow().inner.window_handle().unwrap().as_raw())
    }

    pub fn create_window(
        &mut self,
        acl: &ActiveEventLoop,
        b: WindowAttributes,
    ) -> Arc<WGPUResource> {
        let w = acl.create_window(b).unwrap();

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
                self.ctx.clone(),
                &mut self.event_registry,
            )));
            let gpu = self.main_window.as_ref().unwrap().borrow().gpu();
            gpu
        } else {
            panic!("main window already registered");
            // self.ext_window.push(Window::new(w));
        }
    }

    pub fn run(&mut self) {
        let event_loop = self.event_loop.take().unwrap();
        let err = event_loop.run_app(self);
        if let Err(err) = err {
            error!("{}", err)
        }
    }

    fn process_inner(&self, event: &dyn Any) -> Option<ControlFlow> {
        let source = self.event_source();
        let mut w = self.main_window.as_ref().unwrap().borrow_mut();

        w.on_event(
            &source,
            event,
        );

        self.event_registry.run_event_processor(
            event,
            &source,
        )
    }

    fn process(&self, event_loop: &ActiveEventLoop, event_proxy: &EventLoopProxy<DEvent>, event: &dyn Any) -> Option<ControlFlow> {
        if let Some(c) = event.downcast_ref::<CEvent>() {
            match c {
                core::event::Event::PreUpdate(delta) => {
                    profiling::finish_frame!();
                    // self.has_render_event = true;
                    log::debug!("pre update event");
                    profiling::scope!("pre update");
                    self.frame.lock().unwrap().new_frame();
                    let _ =  event_proxy
                        .send_event(Some(Box::new(CEvent::Update(*delta))));
                    return self.process_inner(event);
                }
                core::event::Event::Update(delta) => {
                    log::debug!("update");
                    profiling::scope!("update event");
                    let _ = 
                        event_proxy
                        .send_event(Some(Box::new(CEvent::PostUpdate(*delta))));
                    return self.process_inner(event);
                }
                core::event::Event::PostUpdate(_) => {
                    // self.has_render_event = false;
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
                log::warn!("EXIT");
                return None;
            }
            if self.auto_exit {
                if let Event::CloseRequested = c {
                    let _ = event_proxy.send_event(Some(Box::new(Event::Exit)));
                    return Some(ControlFlow::Wait);
                }
            }
        }

        self.process_inner(event)
    }
}

impl ApplicationHandler<DEvent> for Looper {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        log::info!("resumed");
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {  
        #[cfg(windows)]
        {
            let w = self.main_window.as_mut().unwrap().borrow_mut();
            w.inner.request_redraw();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: DEvent) {  
        let ctf = self.process(event_loop, &self.src.event_proxy, event.as_ref().unwrap().as_ref()); 
        if let Some(ctf) = ctf {
            event_loop.set_control_flow(ctf);
        } else {
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        event_loop.set_control_flow(ControlFlow::Poll);
        match &event {
            winit::event::WindowEvent::RedrawRequested => {
                let (_, d, ok) = self.frame.lock().unwrap().next_frame();
                if ok {
                    let to_event: Box<dyn Any + Send> =
                        Box::new(CEvent::PreUpdate(d.as_secs_f64()));
                    let _ = self.src.event_proxy.send_event(Some(to_event));
                    event_loop.set_control_flow(ControlFlow::Poll);
                } else {
                    event_loop.set_control_flow(ControlFlow::wait_duration(d));
                }
            }
            _ => {
            },
        }
        let mut w = self.main_window.as_mut().unwrap().borrow_mut();
        let ev = w.on_translate_event(event, &self.src.event_proxy);
        if let Some(ev) = ev {
            drop(w);

            let ctf = self.process(event_loop, &self.src.event_proxy, ev.as_ref().unwrap().as_ref());
            if let Some(ctf) = ctf {
                event_loop.set_control_flow(ctf);
            } else {
                event_loop.exit();
            }
        }
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        match cause {
            StartCause::Init => {
                log::info!("{:?}", cause);
                let attr = self.default_window_attr.take();
                let gpu = self.create_window(event_loop, attr.unwrap());
                self.src.info = Some(EventSourceInformation { gpu });

                let source = self.event_source();

                self.event_registry.run_init(
                    &source,
                );
            }
            StartCause::ResumeTimeReached {
                start: _,
                requested_resume: _,
            } => {
                log::info!("{:?}", cause);
                let w = self.main_window.as_mut().unwrap().borrow_mut();
                w.inner.request_redraw();
            }
            _ => {
            },
        }
    }
}
