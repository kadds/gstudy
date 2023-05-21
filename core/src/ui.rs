use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    backends::wgpu_backend::WGPUResource,
    context::ResourceRef,
    event::{self, *},
    geometry::{load_default_transformer, Mesh},
    types::{Color, Rectu, Size, Vec4f},
    util::{self, any_as_u8_slice_array},
};

pub trait UILogic {
    fn fonts(&self) -> Vec<(String, FontFamily)>;
    fn update(
        &mut self,
        egui_ctx: egui::Context,
        ui_context: &mut UIContext,
        sender: &dyn EventSender,
    );
}

struct EguiRenderFrame {
    pub textures: egui::epaint::textures::TexturesDelta,
    pub shapes: Vec<egui::epaint::ClippedShape>,
}

struct UIInner {
    ctx: Option<egui::Context>,
    ui_logic: Box<dyn UILogic>,
    input: egui::RawInput,
    cursor_position: (f32, f32),
    frame: Option<EguiRenderFrame>,
    cursor: egui::CursorIcon,
    must_render: bool,
    ui_context: Option<Box<UIContext>>,
    ppi: f32,
    clear_color: Option<Color>,
}

impl std::fmt::Debug for UIInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UIInner")
            .field("input", &self.input)
            .field("cursor_position", &self.cursor_position)
            .field("cursor", &self.cursor)
            .field("must_render", &self.must_render)
            .field("ppi", &self.ppi)
            .field("clear_color", &self.clear_color)
            .finish()
    }
}

pub struct UI {
    inner: Arc<Mutex<UIInner>>,
}

impl UI {
    pub fn new(logic: Box<dyn UILogic>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(UIInner::new(logic))),
        }
    }

    pub fn event_processor(&self) -> Box<dyn EventProcessor> {
        Box::new(UIEventProcessor {
            inner: self.inner.clone(),
        })
    }
    pub fn clear_color(&self) -> Color {
        let inner = self.inner.lock().unwrap();
        let ctx = &inner.ctx.as_ref().unwrap();
        let style = ctx.style();
        let raw_window_color = style.visuals.window_fill();
        let target_color = if style.visuals.dark_mode {
            egui::Color32::BLACK
        } else {
            egui::Color32::WHITE
        };
        let rgba: egui::Rgba =
            egui::ecolor::tint_color_towards(raw_window_color, target_color).into();

        let color = match inner.clear_color {
            Some(c) => c,
            None => Color::new(rgba.r(), rgba.g(), rgba.b(), rgba.a()),
        };
        color
    }
}

use egui::FontFamily;
#[cfg(not(target_arch = "wasm32"))]
fn load_font(
    fd: &mut egui::FontDefinitions,
    source: &mut impl font_kit::source::Source,
    name: &str,
    family: FontFamily,
) -> anyhow::Result<()> {
    let font = source.select_best_match(
        &[font_kit::family_name::FamilyName::Title(name.to_string())],
        &font_kit::properties::Properties::new(),
    )?;
    let data = font.load()?;

    fd.font_data.insert(
        name.to_string(),
        egui::FontData::from_owned(
            data.copy_font_data()
                .ok_or(anyhow::Error::msg("load font data fail"))?
                .to_vec(),
        ),
    );
    fd.families
        .entry(family)
        .and_modify(|v| v.insert(0, name.to_string()))
        .or_default();
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn load_fonts(fd: &mut egui::FontDefinitions) {
    let mut s = font_kit::source::SystemSource::new();

    let fonts = vec![
        ("Microsoft YaHei UI", FontFamily::Proportional),
        ("Segoe UI", FontFamily::Proportional),
        ("Consolas", FontFamily::Monospace),
        ("PingFang SC", FontFamily::Proportional),
    ];
    for (name, family) in fonts.into_iter() {
        if let Err(e) = load_font(fd, &mut s, name, family) {
            log::warn!("load font {} fail with {}", name, e);
        }
    }
}

pub struct UIEventProcessor {
    inner: Arc<Mutex<UIInner>>,
}

pub struct UIContext {
    ppi: f32,
}

impl UIInner {
    fn new(logic: Box<dyn UILogic>) -> Self {
        let ctx = egui::Context::default();
        let mut fd = egui::FontDefinitions::default();

        #[cfg(not(target_arch = "wasm32"))]
        load_fonts(&mut fd);

        ctx.set_fonts(fd);

        Self {
            ctx: Some(ctx),
            ui_logic: logic,
            input: egui::RawInput::default(),
            cursor_position: (-1f32, -1f32),
            frame: None,
            cursor: egui::CursorIcon::Default,
            must_render: true,
            ppi: 1.0f32,
            clear_color: None,
            ui_context: Some(Box::new(UIContext { ppi: 1.0f32 })),
        }
    }
}

impl UIEventProcessor {
    fn update(&mut self, proxy: &dyn EventSender, dt: f64) -> ProcessEventResult {
        let mut inner = self.inner.lock().unwrap();

        inner.input.predicted_dt = dt as f32;
        inner.input.pixels_per_point = Some(inner.ppi);

        let ctx = inner.ctx.take().unwrap();
        ctx.begin_frame(inner.input.clone());

        let mut ui_context = inner.ui_context.take().unwrap();
        ui_context.ppi = inner.ppi;

        inner.ui_logic.update(ctx.clone(), &mut ui_context, proxy);

        inner.ui_context = Some(ui_context);

        let output = ctx.end_frame();
        if output.platform_output.cursor_icon != inner.cursor {
            inner.cursor = output.platform_output.cursor_icon;
            let _ = proxy.send_event(Event::UpdateCursor(inner.cursor));
        }
        if !output.platform_output.copied_text.is_empty() {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let err = arboard::Clipboard::new()
                    .and_then(|mut c| c.set_text(output.platform_output.copied_text.clone()));
                if let Err(err) = err {
                    log::error!("{} text {}", err, output.platform_output.copied_text);
                }
            }
        }
        if let Some(url) = output.platform_output.open_url {
            let _ = proxy.send_event(Event::CustomEvent(CustomEvent::OpenUrl(url.url)));
        }
        if let Some(pos) = output.platform_output.text_cursor_pos {
            let _ = proxy.send_event(Event::UpdateImePosition((pos.x as u32, pos.y as u32)));
        }

        inner.frame = Some(EguiRenderFrame {
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
        inner.ctx = Some(ctx);

        ProcessEventResult::Received
    }
}

impl EventProcessor for UIEventProcessor {
    fn on_event(&mut self, source: &dyn EventSource, event: &Event) -> ProcessEventResult {
        match event {
            Event::Update(dt) => {
                return self.update(source.event_sender(), *dt);
            }
            Event::CustomEvent(ev) => match ev {
                CustomEvent::ClearColor(c) => {
                    let mut inner = self.inner.lock().unwrap();
                    inner.clear_color = *c;
                }
                _ => (),
            },
            Event::Render => {}
            Event::Resized { logical, physical } => {
                let mut inner = self.inner.lock().unwrap();
                inner.input.screen_rect = Some(egui::Rect::from_min_max(
                    egui::pos2(0f32, 0f32),
                    egui::pos2(logical.x as f32, logical.y as f32),
                ));
            }
            Event::Input(ev) => match ev {
                InputEvent::KeyboardInput(input) => {
                    let mut inner = self.inner.lock().unwrap();
                    if let Some(key) = event::match_egui_key(input.vk) {
                        let pressed = input.state.is_pressed();
                        if key == egui::Key::C && pressed && inner.input.modifiers.command {
                            inner.input.events.push(egui::Event::Copy);
                        }
                        if key == egui::Key::X && pressed && inner.input.modifiers.command {
                            inner.input.events.push(egui::Event::Cut);
                        }
                        if key == egui::Key::V && pressed && inner.input.modifiers.command {
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                let text = arboard::Clipboard::new()
                                    .and_then(|mut c| c.get_text())
                                    .unwrap_or_default();
                                inner.input.events.push(egui::Event::Paste(text));
                            }
                        }
                        let modifiers = inner.input.modifiers;
                        inner.input.events.push(egui::Event::Key {
                            pressed: input.state.is_pressed(),
                            modifiers,
                            key,
                            repeat: false,
                        });
                    }
                }
                InputEvent::ModifiersChanged(modifiers) => {
                    let mut inner = self.inner.lock().unwrap();
                    let dst = &mut inner.input.modifiers;
                    dst.alt = modifiers.alt;
                    dst.ctrl = modifiers.ctrl;
                    dst.shift = modifiers.shift;

                    if cfg!(targetos = "macos") {
                        dst.mac_cmd = modifiers.win;
                        dst.command = modifiers.win;
                    } else {
                        dst.mac_cmd = false;
                        dst.command = modifiers.ctrl;
                    }
                    log::info!("{:?}", *dst);
                }
                InputEvent::CursorMoved { logical, physical } => {
                    let mut inner = self.inner.lock().unwrap();
                    let replace = if let Some(v) = inner.input.events.last() {
                        match v {
                            egui::Event::PointerMoved(_) => true,
                            _ => false,
                        }
                    } else {
                        false
                    };
                    let ev = egui::Event::PointerMoved(egui::Pos2::new(
                        logical.x as f32,
                        logical.y as f32,
                    ));
                    if replace {
                        *inner.input.events.last_mut().unwrap() = ev;
                    } else {
                        inner.input.events.push(ev)
                    }
                    inner.cursor_position = (logical.x as f32, logical.y as f32);
                }
                InputEvent::ReceivedCharacter(c) => {
                    let c = *c;
                    let mut inner = self.inner.lock().unwrap();
                    if !c.is_ascii_control() {
                        inner.input.events.push(egui::Event::Text(c.to_string()));
                    }
                }
                InputEvent::ReceivedString(s) => {
                    let mut inner = self.inner.lock().unwrap();
                    inner.input.events.push(egui::Event::Text(s.clone()));
                }
                InputEvent::CursorLeft => {
                    let mut inner = self.inner.lock().unwrap();
                    inner.input.events.push(egui::Event::PointerGone);
                }
                InputEvent::MouseWheel { delta } => {
                    let mut inner = self.inner.lock().unwrap();
                    inner
                        .input
                        .events
                        .push(egui::Event::Scroll(egui::vec2(delta.x, delta.y)));
                }
                InputEvent::MouseInput { state, button } => {
                    let mut inner = self.inner.lock().unwrap();
                    let button = match button {
                        event::MouseButton::Left => egui::PointerButton::Primary,
                        event::MouseButton::Right => egui::PointerButton::Secondary,
                        event::MouseButton::Middle => egui::PointerButton::Middle,
                    };
                    let pressed = state.is_pressed();
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
                let mut inner = self.inner.lock().unwrap();
                inner.must_render = true;
            }
            Event::Theme(theme) => {
                let mut inner = self.inner.lock().unwrap();
                match theme {
                    Theme::Light => {
                        inner
                            .ctx
                            .as_ref()
                            .unwrap()
                            .set_visuals(egui::Visuals::light());
                    }
                    Theme::Dark => {
                        inner
                            .ctx
                            .as_ref()
                            .unwrap()
                            .set_visuals(egui::Visuals::dark());
                    }
                }
            }
            Event::ScaleFactorChanged(factor) => {
                let mut inner = self.inner.lock().unwrap();
                inner.ppi = *factor as f32;
                inner.must_render = true;
            }
            _ => (),
        };
        ProcessEventResult::Received
    }
}

#[derive(Default)]
pub struct UITextures {
    textures: HashMap<egui::TextureId, ResourceRef>,
    user_textures: HashMap<egui::TextureId, ResourceRef>,
}

impl UITextures {
    fn update_texture(
        &mut self,
        gpu: &WGPUResource,
        id: egui::TextureId,
        data: egui::epaint::ImageDelta,
    ) {
        let size = data.image.size();
        let mut rect = Rectu::new(0, 0, size[0] as u32, size[1] as u32);
        if let Some(pos) = data.pos {
            rect.x = pos[0] as u32;
            rect.y = pos[1] as u32;
            log::info!("{:?} {:?}", pos, rect);
        } else {
            log::info!("{:?}", rect);
        }

        let size = data.image.size();

        if !self.textures.contains_key(&id) {
            let texture = gpu.new_srgba_2d_texture(
                Some("ui texture"),
                Size::new(size[0] as u32, size[1] as u32),
            );
            let res = gpu.context().register_texture(texture);
            self.textures.insert(id, res);
        }

        let texture = self.textures.get(&id).unwrap();

        match &data.image {
            egui::epaint::ImageData::Color(c) => {
                gpu.copy_texture(
                    texture.texture_ref(),
                    4,
                    rect,
                    any_as_u8_slice_array(&c.pixels),
                );
            }
            egui::epaint::ImageData::Font(f) => {
                let data: Vec<egui::Color32> = f.srgba_pixels(None).collect();
                gpu.copy_texture(texture.texture_ref(), 4, rect, any_as_u8_slice_array(&data));
            }
        }
    }

    pub fn get(&self, texture_id: egui::TextureId) -> ResourceRef {
        self.textures.get(&texture_id).unwrap().clone()
    }
}

#[derive(Debug)]
pub struct UIMesh {}

impl UIMesh {
    pub fn new() -> Self {
        Self {}
    }

    pub fn generate_mesh(
        &self,
        ui: &UI,
        gpu: Arc<WGPUResource>,
        size: Size,
        ui_textures: &mut UITextures,
    ) -> Vec<(Mesh, egui::TextureId)> {
        let mut inner = ui.inner.lock().unwrap();
        let ctx = inner.ctx.take().unwrap();
        let frame = inner.frame.take().unwrap();
        let ppi = ctx.pixels_per_point();

        for (id, data) in frame.textures.set {
            ui_textures.update_texture(&gpu, id, data);
        }

        for id in frame.textures.free {
            ui_textures.textures.remove(&id);
        }

        let meshes = ctx.tessellate(frame.shapes);
        let mut ret = vec![];
        for mesh in meshes {
            let mut clip = if mesh.clip_rect.is_finite() {
                Rectu::new(
                    (mesh.clip_rect.left() * ppi) as u32,
                    (mesh.clip_rect.top() * ppi) as u32,
                    (mesh.clip_rect.right() * ppi) as u32,
                    (mesh.clip_rect.bottom() * ppi) as u32,
                )
            } else {
                Rectu::new(0, 0, 0, 0)
            };

            clip.x = clip.x.max(0);
            clip.y = clip.y.max(0);
            clip.z = clip.z.min(size.x - clip.x);
            clip.w = clip.w.min(size.y - clip.y);

            let mut gmesh = Mesh::new();
            gmesh.set_clip(clip);

            let texture_id = match mesh.primitive {
                egui::epaint::Primitive::Mesh(m) => {
                    gmesh.set_mixed_mesh(
                        any_as_u8_slice_array(&m.vertices),
                        load_default_transformer(),
                    );
                    gmesh.add_indices(&m.indices);
                    m.texture_id
                }
                egui::epaint::Primitive::Callback(_) => todo!(),
            };

            ret.push((gmesh, texture_id));
        }

        inner.ctx = Some(ctx);
        ret
    }
}
