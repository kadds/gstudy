use std::{collections::HashMap, sync::Arc, time::Duration};

use wasm_timer::Instant;
use winit::event_loop::EventLoopProxy;

use crate::{
    event::{CustomEvent, Event},
    render::{executor::TaskId, Canvas},
    types::{Color, Size},
    ui::UIContext,
};

use super::Logic;

struct RenderWindowState {
    canvas: Arc<Canvas>,
    task_id: TaskId,
    texture_id: u64,
    name: String,
    opened: bool,
    new_open: bool,
    closed_time: Option<Instant>,
    progress: f32,
    request_time: Option<Instant>,
}

struct EntryState {
    always_redraw: bool,
    show_settings: bool,
    show_style: bool,
    show_texture: bool,
    show_inspection: bool,
    show_memory: bool,
    show_about: bool,
    about_text: String,
    render_windows: HashMap<TaskId, RenderWindowState>,
    focus_window: Option<TaskId>,
    background: [u8; 3],
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
            render_windows: HashMap::new(),
            focus_window: None,
            background: [0, 0, 0],
        }
    }
}

const DEFAULT_CANVAS_SIZE: [u32; 2] = [256, 256];

pub struct EntryLogic {
    state: EntryState,
}

impl EntryLogic {
    pub fn new() -> Self {
        Self {
            state: EntryState::default(),
        }
    }
}

impl Logic for EntryLogic {
    fn update(
        &mut self,
        ctx: egui::Context,
        ui_context: &mut UIContext,
        proxy: EventLoopProxy<Event>,
    ) {
        let state = &mut self.state;
        egui::SidePanel::left("main_side")
            .min_width(200f32)
            .default_width(300f32)
            .show(&ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {});
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
                    ui.label("background");
                    if ui.color_edit_button_srgb(&mut state.background).changed() {
                        let _ = proxy.send_event(Event::CustomEvent(CustomEvent::ClearColor(
                            Some(Color::new(
                                state.background[0] as f32 / 255f32,
                                state.background[1] as f32 / 255f32,
                                state.background[2] as f32 / 255f32,
                                1f32,
                            )),
                        )));
                    }
                });

                ui.heading("Functions");
                ui.separator();
                let list = ui_context.executor.module_list();
                let tasks = ui_context.executor.tasks();
                egui::ScrollArea::vertical().show_rows(ui, 2.0f32, list.len(), |ui, range| {
                    for idx in range {
                        let module = &list[idx];
                        let label = ui.button(module.name);
                        if label.clicked() {
                            let canvas = Canvas::new(Size::new(
                                (DEFAULT_CANVAS_SIZE[0] as f32 * ui_context.ppi) as u32,
                                (DEFAULT_CANVAS_SIZE[1] as f32 * ui_context.ppi) as u32,
                            ));
                            let id = ui_context.executor.run(idx, canvas.clone());
                            let texture_id = ui_context.add_canvas_and_alloc(canvas.clone());
                            state.render_windows.insert(
                                id,
                                RenderWindowState {
                                    canvas: canvas,
                                    task_id: id,
                                    texture_id,
                                    name: format!("{} {}", module.name, id),
                                    opened: true,
                                    closed_time: None,
                                    new_open: true,
                                    progress: 0f32,
                                    request_time: None,
                                },
                            );
                        }
                        if label.hovered() {
                            egui::show_tooltip(&ctx, egui::Id::new(format!("tt{}", idx)), |ui| {
                                ui.label(module.desc)
                            });
                        }
                    }
                });
            });
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

        let mut closed_window = Vec::new();

        for (id, window_state) in &mut state.render_windows {
            let texture_id = window_state.texture_id;
            let canvas = window_state.canvas.clone();
            let mut opened = window_state.opened;
            let resp = egui::Window::new(&window_state.name)
                .collapsible(true)
                .title_bar(true)
                .open(&mut opened)
                .resizable(true)
                .vscroll(false)
                .default_size([DEFAULT_CANVAS_SIZE[0] as f32, DEFAULT_CANVAS_SIZE[1] as f32])
                .min_width(100f32)
                .min_height(100f32)
                .show(&ctx, |ui| {
                    egui::menu::bar(ui, |ui| {
                        ui.button("pause");
                        ui.button("resume");

                        let download_ok = canvas.download_ok();

                        ui.add_enabled_ui(download_ok.unwrap_or(true), |ui| {
                            if ui.button("snapshot").clicked() {
                                canvas.request_download_texture();
                                window_state.request_time = Some(Instant::now());
                            }
                        });

                        if download_ok.unwrap_or(false) {
                            let file = rfd::FileDialog::new()
                                .add_filter("png", &["png"])
                                .add_filter("tiff", &["tiff"])
                                .add_filter("bmp", &["bmp"])
                                .add_filter("webp", &["webp"])
                                .add_filter("jpeg", &["jpg"])
                                .set_title("save snapshot")
                                .save_file();

                            if let Some(file) = file {
                                let size = canvas.size();
                                let data = canvas.texture_data();
                                let buf = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
                                    size.x, size.y, data,
                                )
                                .unwrap();
                                buf.save(file).unwrap();
                            }
                            canvas.clean_download_state();
                            window_state.request_time = None;
                            window_state.progress = 0f32;
                        }
                        if let Some(t) = window_state.request_time {
                            window_state.progress =
                                1f32 - 1f32 / (Instant::now() - t).as_millis().min(1) as f32;
                        }
                    });
                    let mut available = ui.available_size();
                    if available.y < available.x {
                        available.y = available.x
                    }
                    if available.x < available.y {
                        available.x = available.y
                    }
                    if available.x < DEFAULT_CANVAS_SIZE[0] as f32 {
                        available.x = DEFAULT_CANVAS_SIZE[0] as f32;
                        available.y = DEFAULT_CANVAS_SIZE[1] as f32;
                    }

                    ui.image(
                        egui::TextureId::User(texture_id),
                        available,
                        // egui::vec2(DEFAULT_CANVAS_SIZE[0] as f32, DEFAULT_CANVAS_SIZE[1] as f32),
                    );
                });

            window_state.opened = opened;
            if !window_state.opened {
                if let Some(c) = window_state.closed_time {
                    if Instant::now() - c > Duration::from_secs(1) {
                        closed_window.push(*id);
                    }
                } else {
                    window_state.closed_time = Some(Instant::now());
                }
            }
            if let Some(resp) = resp {
                if window_state.new_open {
                    resp.response.request_focus();
                    window_state.new_open = false;
                }
                if resp.response.has_focus() {
                    state.focus_window = Some(*id);
                }
            }
        }

        for id in closed_window {
            let s = state.render_windows.get(&id).unwrap();
            ui_context.executor.stop(s.task_id);

            state.render_windows.remove(&id);
        }

        if self.state.always_redraw {
            ctx.request_repaint();
        }
    }
}
