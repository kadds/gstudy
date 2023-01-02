use std::{collections::HashMap, sync::Arc, time::Duration};

use egui::Widget;
use instant::Instant;
use winit::{
    dpi::{LogicalPosition, PhysicalPosition},
    event_loop::EventLoopProxy,
};

use crate::{
    core::ps::{PrimitiveStateDescriptor, Topology},
    event::{self, CustomEvent, Event},
    geometry::axis::{Axis, AxisMesh},
    render::{
        material::{
            basic::{BasicMaterialFace, BasicMaterialFaceBuilder},
            MaterialBuilder,
        },
        scene::Object,
        Scene,
    },
    types::{Color, Size, Vec2f, Vec3f, Vec4f},
    ui::UIContext,
};

struct EntryState {
    always_redraw: bool,
    show_settings: bool,
    show_style: bool,
    show_texture: bool,
    show_inspection: bool,
    show_memory: bool,
    show_about: bool,
    about_text: String,
    background: [u8; 3],
    has_background: bool,
}

impl Default for EntryState {
    fn default() -> Self {
        Self {
            always_redraw: false,
            show_settings: false,
            show_style: false,
            show_texture: false,
            show_inspection: false,
            show_memory: false,
            show_about: false,
            about_text: "".to_owned(),
            background: [0, 0, 0],
            has_background: false,
        }
    }
}

pub struct UILogic {
    state: EntryState,
}

impl UILogic {
    pub fn new() -> Self {
        Self {
            state: EntryState::default(),
        }
    }
}

impl UILogic {
    fn main_side(
        &mut self,
        ctx: &egui::Context,
        ui_context: &mut UIContext,
        proxy: EventLoopProxy<Event>,
        ui: &mut egui::Ui,
    ) {
        let state = &mut self.state;
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Load scene").clicked() {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let file = rfd::FileDialog::new()
                            .add_filter("gltf", &["gltf", "glb"])
                            .set_title("load gltf file")
                            .pick_file();

                        if let Some(file) = file {
                            let _ = proxy.send_event(Event::CustomEvent(CustomEvent::Loading(
                                file.to_str().unwrap_or_default().to_string(),
                            )));
                        }
                    }
                    ui.close_menu();
                }
            });
            ui.menu_button("Setting", |ui| {
                ui.checkbox(&mut state.always_redraw, "Always redraw");
                ui.separator();
                ui.checkbox(&mut state.show_settings, "Settings ui");
                ui.checkbox(&mut state.show_style, "Style ui");
                ui.checkbox(&mut state.show_texture, "Texture ui");
                ui.checkbox(&mut state.show_inspection, "Inspection ui");
                ui.checkbox(&mut state.show_memory, "Memory ui");
            });
            ui.menu_button("About", |ui| {
                if ui.button("About me").clicked() {
                    state.show_about = true;
                    ui.close_menu();
                }
            });
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui
                .checkbox(&mut state.has_background, "background")
                .changed()
            {
                let _ = proxy.send_event(if state.has_background {
                    Event::CustomEvent(CustomEvent::ClearColor(Some(Color::new(
                        state.background[0] as f32 / 255f32,
                        state.background[1] as f32 / 255f32,
                        state.background[2] as f32 / 255f32,
                        1f32,
                    ))))
                } else {
                    Event::CustomEvent(CustomEvent::ClearColor(None))
                });
            }
            ui.add_enabled_ui(state.has_background, |ui| {
                if ui.color_edit_button_srgb(&mut state.background).changed() {
                    let _ = proxy.send_event(Event::CustomEvent(CustomEvent::ClearColor(Some(
                        Color::new(
                            state.background[0] as f32 / 255f32,
                            state.background[1] as f32 / 255f32,
                            state.background[2] as f32 / 255f32,
                            1f32,
                        ),
                    ))));
                }
            });
        });
    }

    pub fn update(
        &mut self,
        ctx: egui::Context,
        ui_context: &mut UIContext,
        proxy: EventLoopProxy<Event>,
    ) {
        egui::Window::new("Control")
            .min_width(180f32)
            .default_width(240f32)
            .show(&ctx, |ui| {
                self.main_side(&ctx, ui_context, proxy.clone(), ui);
            });
        let state = &mut self.state;
        egui::Window::new("Settings ui")
            .open(&mut state.show_settings)
            .vscroll(true)
            .show(&ctx, |ui| {
                ctx.settings_ui(ui);
            });
        egui::Window::new("Style ui")
            .open(&mut state.show_style)
            .vscroll(true)
            .show(&ctx, |ui| {
                ctx.style_ui(ui);
            });
        egui::Window::new("Texture ui")
            .open(&mut state.show_texture)
            .vscroll(true)
            .show(&ctx, |ui| {
                ctx.texture_ui(ui);
            });
        egui::Window::new("Inspection ui")
            .open(&mut state.show_inspection)
            .vscroll(true)
            .show(&ctx, |ui| {
                ctx.inspection_ui(ui);
            });
        egui::Window::new("Memory ui")
            .open(&mut state.show_memory)
            .vscroll(true)
            .show(&ctx, |ui| {
                ctx.memory_ui(ui);
            });
        let text = &mut state.about_text;
        egui::Window::new("About")
            .vscroll(true)
            .collapsible(false)
            .fixed_size(&[500f32, 260f32])
            .anchor(egui::Align2::CENTER_CENTER, &[0f32, 0f32])
            .open(&mut state.show_about)
            .show(&ctx, |ui| {
                use egui::special_emojis::*;
                ui.label(egui::RichText::new("GStudy project").heading().strong());
                ui.label(egui::RichText::new(format!(
                    "built by: {} {}\ncommit {} at {}",
                    env!("VERGEN_CARGO_PROFILE"),
                    env!("VERGEN_CARGO_TARGET_TRIPLE"),
                    env!("VERGEN_BUILD_DATE"),
                    env!("VERGEN_GIT_SHA_SHORT")
                )));
                ui.horizontal(|ui| {
                    ui.label("🌞 => ");
                    ui.text_edit_singleline(text);
                });

                ui.separator();
                ui.hyperlink_to(
                    format!("{} github", GITHUB),
                    "https://github.com/kadds/gstudy",
                );
            });

        if self.state.always_redraw {
            ctx.request_repaint();
        }
    }
}

impl UILogic {
    pub fn on_input(&self, ui_context: &UIContext, ev: &event::InputEvent) -> Option<()> {
        Some(())
    }
}
