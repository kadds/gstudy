use std::{cell::RefCell, rc::Rc};

use winit::event_loop::EventLoopProxy;

use crate::{event::*, util};

pub mod egui_renderer;
mod logic;

struct UIInner {
    render: egui_renderer::EguiRenderer,
    ui_logic: Vec<Box<dyn logic::Logic>>,
    input: egui::RawInput,
    cursor_position: (f32, f32),
    frame: Option<egui_renderer::EguiRenderFrame>,
    cursor: egui::CursorIcon,
    must_render: bool,
}

pub struct UI {
    inner: Rc<RefCell<UIInner>>,
}

impl UI {
    pub fn new() -> Self {
        Self {
            inner: RefCell::new(UIInner::new()).into(),
        }
    }
    pub fn event_processor(&self) -> Box<dyn EventProcessor> {
        Box::new(UIEventProcessor {
            inner: self.inner.clone(),
        })
    }
}

pub struct UIEventProcessor {
    inner: Rc<RefCell<UIInner>>,
}

impl UIInner {
    fn new() -> Self {
        let mut ui_logic = Vec::new();
        logic::init(&mut ui_logic);
        Self {
            render: egui_renderer::EguiRenderer::new(),
            ui_logic,
            input: egui::RawInput::default(),
            cursor_position: (-1f32, -1f32),
            frame: None,
            cursor: egui::CursorIcon::Default,
            must_render: true,
        }
    }
}

impl UIEventProcessor {
    fn update(&mut self, proxy: EventLoopProxy<Event>) -> ProcessEventResult {
        let mut inner = self.inner.borrow_mut();
        let ctx = inner.render.ctx();
        ctx.begin_frame(inner.input.clone());
        for logic in &mut inner.ui_logic {
            logic.update(ctx.clone());
        }
        let output = ctx.end_frame();
        if output.platform_output.cursor_icon != inner.cursor {
            inner.cursor = output.platform_output.cursor_icon;
            if let Some(c) = util::match_winit_cursor(inner.cursor) {
                let _ = proxy.send_event(Event::UpdateCursor(c));
            }
        }
        if output.platform_output.copied_text.len() > 0 {
            let err = arboard::Clipboard::new()
                .and_then(|mut c| c.set_text(output.platform_output.copied_text.clone()));
            if let Err(err) = err {
                log::error!("{} text {}", err, output.platform_output.copied_text);
            }
        }
        if let Some(url) = output.platform_output.open_url {
            let _ = proxy.send_event(Event::CustomEvent(CustomEvent::OpenUrl(url.url)));
        }
        if let Some(pos) = output.platform_output.text_cursor_pos {
            let _ = proxy.send_event(Event::UpdateImePosition((pos.x as u32, pos.y as u32)));
        }

        inner.frame = Some(egui_renderer::EguiRenderFrame {
            textures: output.textures_delta,
            shapes: output.shapes,
        });
        if output.needs_repaint || inner.must_render {
            let _ = proxy.send_event(Event::Render);
            inner.must_render = false;
        }

        return ProcessEventResult::Received;
    }
}

impl EventProcessor for UIEventProcessor {
    fn on_event(&mut self, source: &dyn EventSource, event: &Event) -> ProcessEventResult {
        match event {
            Event::Update => {
                return self.update(source.event_proxy());
            }
            Event::Render => {
                let mut inner = self.inner.borrow_mut();
                let frame = inner.frame.take().unwrap();
                inner.render.render(source.backend(), frame);
                // clear all input
                inner.input.dropped_files.clear();
                inner.input.events.clear();
                inner.input.hovered_files.clear();
            }
            Event::Resized(size) => {
                let mut inner = self.inner.borrow_mut();
                inner.input.screen_rect = Some(egui::Rect::from_min_max(
                    egui::pos2(0f32, 0f32),
                    egui::pos2(size.x as f32, size.y as f32),
                ));
                inner.render.resize(*size);
            }
            Event::Input(ev) => match ev {
                InputEvent::KeyboardInput {
                    device_id,
                    input,
                    is_synthetic,
                } => {
                    let mut inner = self.inner.borrow_mut();
                    if let Some(key) = util::match_egui_key(
                        input
                            .virtual_keycode
                            .unwrap_or(winit::event::VirtualKeyCode::Apostrophe),
                    ) {
                        let pressed = input.state == winit::event::ElementState::Pressed;
                        if key == egui::Key::C && pressed && inner.input.modifiers.command {
                            inner.input.events.push(egui::Event::Copy);
                        }
                        if key == egui::Key::X && pressed && inner.input.modifiers.command {
                            inner.input.events.push(egui::Event::Cut);
                        }
                        if key == egui::Key::V && pressed && inner.input.modifiers.command {
                            let text = arboard::Clipboard::new()
                                .and_then(|mut c| c.get_text())
                                .unwrap_or_default();
                            inner.input.events.push(egui::Event::Paste(text));
                        }
                        let modifiers = inner.input.modifiers.clone();
                        inner.input.events.push(egui::Event::Key {
                            pressed: input.state == winit::event::ElementState::Pressed,
                            modifiers,
                            key,
                        });
                    }
                }
                InputEvent::ModifiersChanged(modifiers) => {
                    let mut inner = self.inner.borrow_mut();
                    let dst = &mut inner.input.modifiers;
                    dst.alt = modifiers.alt();
                    dst.ctrl = modifiers.ctrl();
                    dst.shift = modifiers.shift();

                    if cfg!(targetos = "macos") {
                        dst.mac_cmd = modifiers.logo();
                        dst.command = modifiers.logo();
                    } else {
                        dst.mac_cmd = false;
                        dst.command = modifiers.ctrl();
                    }
                    log::info!("{:?}", *dst);
                }
                InputEvent::CursorMoved {
                    device_id,
                    position,
                } => {
                    let mut inner = self.inner.borrow_mut();
                    inner
                        .input
                        .events
                        .push(egui::Event::PointerMoved(egui::Pos2::new(
                            position.x as f32,
                            position.y as f32,
                        )));
                    inner.cursor_position = (position.x as f32, position.y as f32);
                }
                InputEvent::ReceivedCharacter(c) => {
                    let c = *c;
                    let mut inner = self.inner.borrow_mut();
                    if !c.is_ascii_control() {
                        inner.input.events.push(egui::Event::Text(c.to_string()));
                    }
                }
                InputEvent::CursorLeft { device_id } => {
                    let mut inner = self.inner.borrow_mut();
                    inner.input.events.push(egui::Event::PointerGone);
                }
                InputEvent::MouseWheel {
                    device_id,
                    delta,
                    phase,
                } => {
                    let mut inner = self.inner.borrow_mut();
                    inner.input.events.push(egui::Event::Scroll(match *delta {
                        winit::event::MouseScrollDelta::LineDelta(x, y) => {
                            egui::Vec2::new(x * 50f32, y * 50f32)
                        }
                        winit::event::MouseScrollDelta::PixelDelta(a) => {
                            egui::Vec2::new(a.x as f32, a.y as f32)
                        }
                    }));
                }
                InputEvent::MouseInput {
                    device_id,
                    state,
                    button,
                } => {
                    let mut inner = self.inner.borrow_mut();
                    let button = match button {
                        winit::event::MouseButton::Left => egui::PointerButton::Primary,
                        winit::event::MouseButton::Right => egui::PointerButton::Secondary,
                        winit::event::MouseButton::Middle => egui::PointerButton::Middle,
                        winit::event::MouseButton::Other(_) => {
                            return ProcessEventResult::Received;
                        }
                    };
                    let pressed = match state {
                        winit::event::ElementState::Pressed => true,
                        winit::event::ElementState::Released => false,
                    };
                    let pos = inner.cursor_position;
                    let modifiers = inner.input.modifiers.clone();
                    inner.input.events.push(egui::Event::PointerButton {
                        pos: egui::pos2(pos.0, pos.1),
                        modifiers,
                        pressed,
                        button,
                    });
                }
                _ => (),
            },
            Event::JustRenderOnce => {
                let mut inner = self.inner.borrow_mut();
                inner.must_render = true;
            },
            _ => (),
        };
        ProcessEventResult::Received
    }
}
