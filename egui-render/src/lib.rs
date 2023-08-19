mod mesh;

use core::{
    backends::wgpu_backend::WGPUResource,
    context::{RContext, RContextRef, TagId},
    event::{EventProcessor, EventSender, EventSource, InputEvent, ProcessEventResult},
    geometry::StaticGeometry,
    material::{Material, MaterialBuilder},
    scene::{Camera, RenderObject, Scene},
    types::{Size, Vec3f, Vec4f},
};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Arc,
};

type CEvent = core::event::Event;

use app::{
    container::Container,
    plugin::{CoreFactoryList, Plugin, PluginFactory},
    AppEventProcessor,
};
use egui::FontFamily;
use material::{EguiMaterialFace, EguiMaterialFaceBuilder};
use mesh::{UIMesh, UITextures};
use util::load_font;
use window::WindowSize;

use crate::material_render::EguiMaterialRendererFactory;

pub mod material;
pub mod material_render;
pub mod render;
mod util;

pub struct EguiRenderer {
    ui_textures: Option<UITextures>,
    ui_mesh: UIMesh,

    ui_materials: Option<HashMap<egui::TextureId, Arc<Material>>>,
    ui_tag: TagId,

    ctx: Arc<egui::Context>,
    input: egui::RawInput,
    cursor_position: (f32, f32),
    frame: Option<EguiRenderFrame>,
    cursor: egui::CursorIcon,
    must_render: bool,
    // ui_context: Option<Box<UIContext>>,
    ppi: f32,
    has_udpate: bool,
}

impl EguiRenderer {
    pub fn new(context: &RContext) -> Self {
        let ui_tag = context.new_tag("egui-element");
        let ctx = egui::Context::default();
        let mut s = Self {
            ui_mesh: UIMesh::new(ctx.clone()),
            ui_textures: Some(UITextures::default()),
            ui_materials: Some(HashMap::new()),
            ui_tag,

            ctx: Arc::new(ctx),
            input: egui::RawInput::default(),
            cursor_position: (0f32, 0f32),
            frame: None,
            cursor: egui::CursorIcon::default(),
            must_render: true,
            ppi: 1.0f32,
            has_udpate: false,
        };
        s.set_default_fonts();

        s
    }

    pub fn set_default_fonts(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut s = font_kit::source::SystemSource::new();

            let fonts = vec![
                ("Microsoft YaHei UI", FontFamily::Proportional),
                ("Segoe UI", FontFamily::Proportional),
                ("Consolas", FontFamily::Monospace),
                ("PingFang SC", FontFamily::Proportional),
                ("Source Han Sans CN", FontFamily::Proportional),
                ("WenQuanYi Zen Hei Mono", FontFamily::Proportional),
                ("Source code Pro", FontFamily::Monospace),
            ];
            let mut fd = egui::FontDefinitions::empty();

            for (name, family) in fonts.into_iter() {
                if let Err(e) = load_font(&mut fd, &mut s, name, family) {
                    log::warn!("load font {} fail {}", name, e);
                } else {
                    log::info!("load font {} ready", name);
                }
            }
            self.ctx.set_fonts(fd);
        }
    }

    pub fn pre_update(&mut self, dt: f32, size: Size) {
        self.input.predicted_dt += dt as f32;
        self.input.pixels_per_point = Some(self.ppi);
        self.input.screen_rect = Some(egui::Rect::from_min_max(
            egui::pos2(0f32, 0f32),
            egui::pos2(size.x as f32, size.y as f32), // logical size
        ));

        self.ctx.begin_frame(self.input.clone());
    }

    pub fn post_update(&mut self, proxy: &dyn EventSender, scene: &Scene) {
        let output = self.ctx.end_frame();
        if output.platform_output.cursor_icon != self.cursor {
            self.cursor = output.platform_output.cursor_icon;
            proxy.send_event(Box::new(window::Event::UpdateCursor(
                util::match_winit_cursor(self.cursor).unwrap(),
            )));
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
            proxy.send_event(Box::new(window::Event::OpenUrl(url.url)));
        }
        if let Some(pos) = output.platform_output.text_cursor_pos {
            proxy.send_event(Box::new(window::Event::UpdateImePosition((
                pos.x as u32,
                pos.y as u32,
            ))));
        }

        if let Some(f) = self.frame.as_mut() {
            f.textures.push(output.textures_delta);
            f.shapes = output.shapes;
        } else {
            self.frame = Some(EguiRenderFrame {
                textures: vec![output.textures_delta],
                shapes: output.shapes,
            })
        }
        // clear all input
        self.input.dropped_files.clear();
        self.input.events.clear();
        self.input.hovered_files.clear();

        self.input.predicted_dt = 0f32;
        self.has_udpate = true;
    }

    pub fn pre_render(&mut self, gpu: Arc<WGPUResource>, scene: &Scene, view_size: Size) {
        if !self.has_udpate {
            return;
        }

        self.has_udpate = false;

        scene.remove_by_tag(self.ui_tag);

        let mut ui_materials = self.ui_materials.take().unwrap();
        // let size = self
        //     .input
        //     .screen_rect
        //     .map(|v| Size::new(v.width() as u32, v.height() as u32))
        //     .unwrap();

        let mut ui_textures = self.ui_textures.take().unwrap();
        let (meshes, rebuild_textures) = self.ui_mesh.generate_mesh(
            self.frame.take().unwrap(),
            gpu.clone(),
            view_size,
            &mut ui_textures,
        );

        for texture_id in rebuild_textures {
            ui_materials.remove(&texture_id);
        }

        for (mesh, texture_id) in meshes {
            let material = ui_materials.entry(texture_id).or_insert_with(|| {
                let t = ui_textures.get(texture_id);
                MaterialBuilder::default()
                    .with_face(EguiMaterialFaceBuilder::default().with_texture(t).build())
                    .build(&scene.context())
            });

            let mut object = RenderObject::new(
                Box::new(StaticGeometry::new(Arc::new(mesh))),
                material.clone(),
            );
            object.add_tag(self.ui_tag);

            scene.add_ui(object);
        }

        self.ui_textures = Some(ui_textures);
        self.ui_materials = Some(ui_materials);
    }

    fn on_event(&mut self, source: &dyn EventSource, event: &dyn Any) -> ProcessEventResult {
        if let Some(cevent) = event.downcast_ref::<CEvent>() {
            match &cevent {
                CEvent::Input(ev) => match ev {
                    InputEvent::KeyboardInput(input) => {
                        if let Some(key) = util::match_egui_vkey(input.vk) {
                            let pressed = input.state.is_pressed();
                            if key == egui::Key::C && pressed && self.input.modifiers.command {
                                self.input.events.push(egui::Event::Copy);
                            }
                            if key == egui::Key::X && pressed && self.input.modifiers.command {
                                self.input.events.push(egui::Event::Cut);
                            }
                            if key == egui::Key::V && pressed && self.input.modifiers.command {
                                #[cfg(not(target_arch = "wasm32"))]
                                {
                                    let text = arboard::Clipboard::new()
                                        .and_then(|mut c| c.get_text())
                                        .unwrap_or_default();
                                    self.input.events.push(egui::Event::Paste(text));
                                }
                            }
                            let modifiers = self.input.modifiers;
                            self.input.events.push(egui::Event::Key {
                                pressed: input.state.is_pressed(),
                                modifiers,
                                key,
                                repeat: false,
                            });
                        }
                    }
                    InputEvent::ModifiersChanged(modifiers) => {
                        let dst = &mut self.input.modifiers;
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
                    }
                    InputEvent::CursorMoved { logical, physical } => {
                        let replace = if let Some(v) = self.input.events.last() {
                            match v {
                                egui::Event::PointerMoved(_) => true,
                                _ => false,
                            }
                        } else {
                            false
                        };
                        let ev = egui::Event::PointerMoved(egui::Pos2::new(logical.x, logical.y));
                        if replace {
                            *self.input.events.last_mut().unwrap() = ev;
                        } else {
                            self.input.events.push(ev)
                        }
                        self.cursor_position = (logical.x, logical.y);
                    }
                    InputEvent::ReceivedCharacter(c) => {
                        let c = *c;
                        if !c.is_ascii_control() {
                            self.input.events.push(egui::Event::Text(c.to_string()));
                        }
                    }
                    InputEvent::ReceivedString(s) => {
                        self.input.events.push(egui::Event::Text(s.clone()));
                    }
                    InputEvent::CursorLeft => {
                        self.input.events.push(egui::Event::PointerGone);
                    }
                    InputEvent::MouseWheel { delta } => {
                        self.input
                            .events
                            .push(egui::Event::Scroll(egui::vec2(delta.x, delta.y)));
                    }
                    InputEvent::MouseInput { state, button } => {
                        let button = match button {
                            core::event::MouseButton::Left => egui::PointerButton::Primary,
                            core::event::MouseButton::Right => egui::PointerButton::Secondary,
                            core::event::MouseButton::Middle => egui::PointerButton::Middle,
                        };
                        let pressed = state.is_pressed();
                        let pos = self.cursor_position;
                        let modifiers = self.input.modifiers;
                        self.input.events.push(egui::Event::PointerButton {
                            pos: egui::pos2(pos.0, pos.1),
                            modifiers,
                            pressed,
                            button,
                        });
                    }
                    _ => (),
                },
                _ => (),
            };
        } else if let Some(wevent) = event.downcast_ref::<window::Event>() {
            match wevent {
                window::Event::ScaleFactorChanged(factor) => {
                    self.ppi = *factor as f32;
                    log::info!("egui ppi {:?}", self.ppi);
                    self.must_render = true;
                }
                _ => (),
            }
        }
        ProcessEventResult::Received
    }
}

#[derive(Debug)]
pub enum Event {}

struct EguiRenderFrame {
    pub textures: Vec<egui::epaint::textures::TexturesDelta>,
    pub shapes: Vec<egui::epaint::ClippedShape>,
}

pub struct EguiPluginFactory;

impl PluginFactory for EguiPluginFactory {
    fn create(&self, container: &Container) -> Box<dyn Plugin> {
        Box::new(EguiPlugin::new(container))
    }

    fn info(&self) -> app::plugin::PluginInfo {
        app::plugin::PluginInfo {
            name: "egui".into(),
            version: "0.1.0".into(),
            has_looper: false,
        }
    }
}

pub struct EguiPlugin {
    r: EguiRenderer,
}

impl EguiPlugin {
    pub fn new(container: &Container) -> Self {
        let context = container.get::<RContext>().unwrap();
        let s = Self {
            r: EguiRenderer::new(&context),
        };

        let ctx = s.r.ctx.clone();
        container.register_arc(ctx);
        s
    }
}

impl Plugin for EguiPlugin {
    fn load_factory(&self) -> CoreFactoryList {
        let list = CoreFactoryList {
            materials: vec![(
                TypeId::of::<EguiMaterialFace>(),
                Box::new(EguiMaterialRendererFactory {}),
            )],
        };
        list
    }
}

impl AppEventProcessor for EguiPlugin {
    fn on_event(&mut self, context: &app::AppEventContext, event: &dyn Any) {
        if let Some(ev) = event.downcast_ref::<core::event::Event>() {
            match ev {
                core::event::Event::PreUpdate(delta) => {
                    let size = context.container.get::<WindowSize>().unwrap().get().1;
                    self.r.pre_update(*delta as f32, size);
                }
                core::event::Event::PostUpdate(_) => {
                    let scene = context.container.get::<Scene>().unwrap();
                    self.r.post_update(context.source.event_sender(), &scene);
                }
                core::event::Event::PreRender => {
                    let gpu = context.container.get::<WGPUResource>().unwrap();
                    let scene = context.container.get::<Scene>().unwrap();
                    let view_size = context.container.get::<WindowSize>().unwrap().get().0;
                    self.r.pre_render(gpu, &scene, view_size);
                }
                _ => {
                    self.r.on_event(context.source, event);
                }
            }
        } else {
            self.r.on_event(context.source, event);
        }
    }
}
