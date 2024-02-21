use core::{
    backends::{
        wgpu_backend::{WGPUResource, WindowSurfaceFrame2},
        WGPUBackend,
    },
    context::RContextRef,
    event::{EventRegistry, EventSender, InputEvent},
    types::{Size, Vec2f, Vec3f},
};
use std::{any::Any, sync::Arc};

use winit::{
    dpi::{LogicalPosition, LogicalSize},
    event::WindowEvent,
    event_loop::EventLoopProxy,
};

use crate::{util, CEvent, DEvent, Event, Theme, WEvent};

pub struct Window {
    pub(crate) inner: winit::window::Window,
    pub(crate) backend: WGPUBackend,
    pub(crate) gpu: Arc<WGPUResource>,
    pub(crate) size: Size,
    first_render: bool,
    frame: Option<WindowSurfaceFrame2>,
    has_resize_event: bool,
    pub(crate) delay_frame_ms: u64,
}

impl Window {
    pub fn new(
        w: winit::window::Window,
        context: RContextRef,
        event_registry: &mut dyn EventRegistry,
    ) -> Self {
        let size = w.inner_size();
        log::info!("init window size {:?}", size);

        let backend = WGPUBackend::new(&w, size.width, size.height, context).unwrap();
        let gpu = backend.gpu();
        event_registry.register_processor(backend.event_processor());

        Self {
            inner: w,
            backend,
            gpu,
            size: Size::new(size.width, size.height),
            first_render: true,
            frame: None,
            has_resize_event: false,
            delay_frame_ms: 0,
        }
    }

    pub fn gpu(&self) -> Arc<WGPUResource> {
        self.gpu.clone()
    }

    pub fn fullscreen(&self, ev: Event) {
        if let Event::FullScreen(fullscreen) = ev {
            if !fullscreen {
                self.inner.set_fullscreen(None);
            } else {
                self.inner
                    .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
            }
        }
    }

    pub fn on_event(&mut self, proxy: &dyn EventSender, ev: &dyn Any) {
        if let Some(ev) = ev.downcast_ref::<CEvent>() {
            match ev {
                core::event::Event::PostUpdate(_) => {
                    log::debug!("post update");
                    proxy.send_event(Box::new(core::event::Event::PreRender));
                }
                core::event::Event::PreRender => {
                    log::debug!("pre render");
                    if self.frame.is_some() {
                        return;
                    }
                    let wait_ts = instant::Instant::now();
                    let surface_frame = match self.gpu.clone().current_frame_texture2() {
                        Ok(v) => v,
                        Err(e) => {
                            log::error!("{}", e);
                            return;
                        }
                    };
                    let wait_end = instant::Instant::now();
                    let wait_ms = (wait_end - wait_ts).as_millis() as u64;
                    if wait_ms > 4 {
                        log::warn!("get swapchain delay {}ms", wait_ms);
                    }
                    self.delay_frame_ms = wait_ms;

                    proxy.send_event(Box::new(core::event::Event::Render(
                        surface_frame.texture(),
                    )));
                    self.frame = Some(surface_frame);
                }
                core::event::Event::Render(_) => {
                    log::debug!("render");
                    proxy.send_event(Box::new(core::event::Event::PostRender));
                }
                core::event::Event::PostRender => {
                    if self.first_render {
                        self.first_render = false;
                        self.inner.set_visible(true);
                    }
                    self.frame = None;
                    log::debug!("post render");
                    self.before_update(proxy);
                }
                core::event::Event::Resized {
                    logical: _,
                    physical: _,
                } => {
                    self.frame = None;
                }
                _ => (),
            }
        }
    }

    fn before_update(&mut self, proxy: &dyn EventSender) {
        if self.has_resize_event {
            let size = self.inner.inner_size();
            let logical: LogicalSize<u32> = size.to_logical(self.inner.scale_factor());
            log::info!("window resize {:?}", size);
            let e = CEvent::Resized {
                physical: Size::new(size.width.max(1), size.height.max(1)),
                logical: Size::new(logical.width.max(1), logical.height.max(1)),
            };
            proxy.send_event(Box::new(e));
            self.has_resize_event = false;
        }
    }

    pub fn on_translate_event(
        &mut self,
        original_event: WEvent,
        _proxy: &EventLoopProxy<DEvent>,
    ) -> Option<DEvent> {
        match original_event {
            WEvent::WindowEvent {
                event,
                window_id: _,
            } => {
                if let WindowEvent::Resized(_) = &event {
                    let size = self.inner.inner_size();
                    self.size = Size::new(size.width.max(1), size.height.max(1));
                    self.has_resize_event = true;
                }

                let ev = map_event(&self.inner, event);
                return ev;
            }
            WEvent::NewEvents(cause) => match cause {
                winit::event::StartCause::ResumeTimeReached {
                    start: _,
                    requested_resume: _,
                } => {
                    self.inner.request_redraw();
                }
                winit::event::StartCause::Init => {
                    let scale = self.inner.scale_factor();
                    log::info!("init window scale {}", scale);
                    self.inner.set_visible(true);
                    return Some(Box::new(Event::ScaleFactorChanged(scale)));
                }
                _ => {}
            },
            _ => (),
        }
        None
    }
}

fn map_event(w: &winit::window::Window, event: WindowEvent) -> Option<DEvent> {
    Some(Box::new(match event {
        WindowEvent::Resized(_) => {
            return None;
        }
        // WindowEvent::Moved(pos) => Event::Moved(Size::new(pos.x as u32, pos.y as u32)),
        WindowEvent::CloseRequested => return Some(Box::new(Event::CloseRequested)),
        WindowEvent::Focused(f) => return Some(Box::new(Event::Focused(f))),
        ev => CEvent::Input(match ev {
            // WindowEvent::ReceivedCharacter(c) => InputEvent::ReceivedCharacter(c),
            WindowEvent::Ime(ime) => match ime {
                winit::event::Ime::Commit(s) => InputEvent::ReceivedString(s),
                _ => {
                    return None;
                }
            },
            WindowEvent::KeyboardInput {
                device_id: _,
                event,
                is_synthetic: _,
            } => InputEvent::KeyboardInput(core::event::KeyboardInput {
                state: util::match_state(event.state),
                vk: util::match_vk(event.physical_key),
            }),
            WindowEvent::ModifiersChanged(state) => {
                InputEvent::ModifiersChanged(core::event::ModifiersState {
                    ctrl: state.state().control_key(),
                    win: state.state().super_key(),
                    alt: state.state().alt_key(),
                    shift: state.state().shift_key(),
                })
            }
            WindowEvent::CursorMoved {
                device_id: _,
                position,
            } => {
                let logical: LogicalPosition<u32> = position.to_logical(w.scale_factor());
                InputEvent::CursorMoved {
                    physical: Vec2f::new(position.x as f32, position.y as f32),
                    logical: Vec2f::new(logical.x as f32, logical.y as f32),
                }
            }
            WindowEvent::CursorEntered { device_id: _ } => InputEvent::CursorEntered,
            WindowEvent::CursorLeft { device_id: _ } => InputEvent::CursorLeft,
            WindowEvent::MouseWheel {
                device_id: _,
                delta,
                phase: _,
            } => InputEvent::MouseWheel {
                delta: match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        Vec3f::new(x * 20f32, y * 20f32, 0f32)
                    }
                    winit::event::MouseScrollDelta::PixelDelta(p) => {
                        Vec3f::new(p.x as f32 * 20f32, p.y as f32 * 20f32, 0f32)
                    }
                },
            },
            WindowEvent::MouseInput {
                device_id: _,
                state,
                button,
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
                return Some(Box::new(match theme {
                    winit::window::Theme::Light => Event::Theme(Theme::Light),
                    winit::window::Theme::Dark => Event::Theme(Theme::Dark),
                }));
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                inner_size_writer: _,
            } => {
                log::info!("scale factor changed {}", scale_factor);
                return Some(Box::new(Event::ScaleFactorChanged(scale_factor)));
            }
            _ => {
                return None;
            }
        }),
    }))
}
