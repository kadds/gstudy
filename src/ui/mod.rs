use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event_loop::EventLoopProxy,
};

use crate::{
    event::*,
    render::{Canvas, Executor},
    types::{Color, Size},
    util,
};

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
    ui_context: Option<Box<UIContext>>,
    ppi: f32,
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

pub struct UIContext {
    pub executor: Executor,
    canvas_map: HashMap<u64, Arc<Canvas>>,
    last_texture_id: u64,
}

impl UIContext {
    pub fn alloc(&mut self) -> u64 {
        self.last_texture_id += 1;
        self.last_texture_id
    }
    pub fn add_canvas_and_alloc(&mut self, canvas: Arc<Canvas>) -> u64 {
        let id = self.alloc();
        self.canvas_map.insert(id, canvas);
        id
    }
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
            ppi: 1.0f32,
            ui_context: Some(Box::new(UIContext {
                executor: Executor::new(),
                canvas_map: HashMap::new(),
                last_texture_id: 0,
            })),
        }
    }
}

impl UIEventProcessor {
    fn update(&mut self, proxy: EventLoopProxy<Event>, dt: f64) -> ProcessEventResult {
        let mut inner = self.inner.borrow_mut();
        let ctx = inner.render.ctx();
        inner.input.predicted_dt = dt as f32;
        inner.input.pixels_per_point = Some(inner.ppi);

        ctx.begin_frame(inner.input.clone());
        let mut ui_context = inner.ui_context.take().unwrap();
        for logic in &mut inner.ui_logic {
            logic.update(ctx.clone(), &mut ui_context);
        }
        ui_context.executor.update();
        inner.ui_context = Some(ui_context);

        let output = ctx.end_frame();
        if output.platform_output.cursor_icon != inner.cursor {
            inner.cursor = output.platform_output.cursor_icon;
            if let Some(c) = util::match_winit_cursor(inner.cursor) {
                let _ = proxy.send_event(Event::UpdateCursor(c));
            }
        }
        if !output.platform_output.copied_text.is_empty() {
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
        if output.repaint_after.is_zero() || inner.must_render {
            let _ = proxy.send_event(Event::Render);
            inner.must_render = false;
        }

        // clear all input
        inner.input.dropped_files.clear();
        inner.input.events.clear();
        inner.input.hovered_files.clear();

        ProcessEventResult::Received
    }
}

impl EventProcessor for UIEventProcessor {
    fn on_event(&mut self, source: &dyn EventSource, event: &Event) -> ProcessEventResult {
        match event {
            Event::Update(dt) => {
                return self.update(source.event_proxy(), *dt);
            }
            Event::Render => {
                let mut inner = self.inner.borrow_mut();
                let ctx = inner.render.ctx();
                let style = ctx.style();
                let frame = inner.frame.take().unwrap();
                let raw_window_color = style.visuals.window_fill();
                let target_color = if style.visuals.dark_mode {
                    egui::Color32::BLACK
                } else {
                    egui::Color32::WHITE
                };
                let rgba: egui::Rgba =
                    egui::color::tint_color_towards(raw_window_color, target_color).into();

                let color = Color::new(rgba.r(), rgba.g(), rgba.b(), rgba.a());

                let mut ui_context = inner.ui_context.take().unwrap();

                let ppi = inner.ppi;
                inner
                    .render
                    .render(source.backend(), frame, color, ppi, &mut ui_context);
                inner.ui_context = Some(ui_context);
            }
            Event::Resized(_) => {
                let mut inner = self.inner.borrow_mut();
                let size: PhysicalSize<u32> = source.window().inner_size();
                let logic_size: LogicalSize<u32> = size.to_logical(source.window().scale_factor());
                inner.input.screen_rect = Some(egui::Rect::from_min_max(
                    egui::pos2(0f32, 0f32),
                    egui::pos2(logic_size.width as f32, logic_size.height as f32),
                ));
                inner
                    .render
                    .resize(Size::new(logic_size.width, logic_size.height));
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
                        let modifiers = inner.input.modifiers;
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
                    let position: winit::dpi::LogicalPosition<f64> =
                        position.to_logical(source.window().scale_factor());
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
                    let modifiers = inner.input.modifiers;
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
            }
            Event::Theme(theme) => {
                let inner = self.inner.borrow_mut();
                match theme {
                    Theme::Light => {
                        inner.render.ctx().set_visuals(egui::Visuals::light());
                    }
                    Theme::Dark => {
                        inner.render.ctx().set_visuals(egui::Visuals::dark());
                    }
                }
            }
            Event::ScaleFactorChanged(factor) => {
                let mut inner = self.inner.borrow_mut();
                inner.ppi = *factor as f32;
                inner.must_render = true;
            }
            _ => (),
        };
        ProcessEventResult::Received
    }
}
