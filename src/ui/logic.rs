use std::{collections::HashMap, sync::Arc, time::Duration};

use egui::Widget;
use instant::Instant;
use winit::{
    dpi::{LogicalPosition, PhysicalPosition},
    event_loop::EventLoopProxy,
};

use crate::{
    event::{self, CustomEvent, Event},
    modules::hardware_renderer::common::Position,
    render::{
        executor::{ExecutorInputEvent, TaskId},
        Canvas, Scene,
    },
    types::{Color, Size, Vec2f, Vec4f},
    ui::UIContext,
};

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
    rect: Vec4f,
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
            render_windows: HashMap::new(),
            focus_window: None,
            background: [0, 0, 0],
            has_background: false,
        }
    }
}

const DEFAULT_CANVAS_SIZE: [u32; 2] = [512, 512];

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

        ui.heading("Functions");
        ui.separator();
        let list = ui_context.executor.module_list();
        egui::ScrollArea::vertical().show_rows(ui, 2.0f32, list.len(), |ui, range| {
            for idx in range {
                let module = &list[idx];
                let label = ui.button(module.name);
                if label.clicked() {
                    self.add_scene(ui_context, Scene::new());
                }
                if label.hovered() {
                    egui::show_tooltip(&ctx, egui::Id::new(format!("tt{}", idx)), |ui| {
                        ui.label(module.desc)
                    });
                }
            }
        });
    }

    fn render_window_ui(
        ctx: &egui::Context,
        ui_context: &mut UIContext,
        proxy: EventLoopProxy<Event>,
        window_state: &mut RenderWindowState,
        ui: &mut egui::Ui,
    ) {
        let canvas = window_state.canvas.clone();
        let texture_id = window_state.texture_id;
        egui::menu::bar(ui, |ui| {
            ui.menu_button("control", |ui| {
                ui.button("trackball camera");
                ui.button("xxx camera");
            });
            ui.menu_button("scene", |ui| {
                ui.button("pause");
                ui.button("resume");
                ui.separator();
                ui.button("edit mode");
                ui.separator();
                ui.button("resize canvas");
            });

            let download_ok = canvas.download_ok();

            ui.add_enabled_ui(download_ok.unwrap_or(true), |ui| {
                if ui.button("snapshot").clicked() {
                    canvas.request_download_texture();
                    window_state.request_time = Some(Instant::now());
                }
            });

            if download_ok.unwrap_or(false) {
                #[cfg(not(target_arch = "wasm32"))]
                {
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

        let resp = egui::Image::new(egui::TextureId::User(texture_id), available)
            .sense(egui::Sense::click_and_drag().union(egui::Sense::focusable_noninteractive()))
            .ui(ui);
        window_state.rect = Vec4f::new(
            resp.rect.left(),
            resp.rect.top(),
            resp.rect.right(),
            resp.rect.bottom(),
        );
    }

    pub fn update(
        &mut self,
        ctx: egui::Context,
        ui_context: &mut UIContext,
        proxy: EventLoopProxy<Event>,
    ) {
        egui::SidePanel::left("main_side")
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

        let mut closed_window = Vec::new();
        let mut any_dirty = false;

        for (id, window_state) in &mut state.render_windows {
            if !any_dirty && window_state.canvas.dirty() {
                any_dirty = true;
            }
            let proxy2 = proxy.clone();

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
                    Self::render_window_ui(&ctx, ui_context, proxy2, window_state, ui);
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
                if resp.response.drag_started() || resp.response.clicked() {
                    state.focus_window = Some(*id);
                }
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

        if self.state.always_redraw || any_dirty {
            ctx.request_repaint();
        }
    }
}

impl UILogic {
    pub fn on_input(&self, ui_context: &UIContext, ev: &event::InputEvent) -> Option<()> {
        let id = self.state.focus_window?;
        let window = self.state.render_windows.get(&id)?;
        let ppi = ui_context.ppi;
        match ev {
            event::InputEvent::KeyboardInput {
                device_id,
                input,
                is_synthetic,
            } => {
                ui_context.executor.send_input(id, ev.clone());
            }
            event::InputEvent::CursorMoved {
                device_id,
                position,
            } => {
                let position: LogicalPosition<f64> = position.to_logical(ppi as f64);
                let x = position.x as f32 - window.rect.x;
                let y = position.y as f32 - window.rect.y;
                if x < 0f32 || y < 0f32 || x > window.rect.z || y > window.rect.w {
                    return None;
                }

                let input = event::InputEvent::CursorMoved {
                    device_id: device_id.clone(),
                    position: LogicalPosition::new(x as f64, y as f64).to_physical(ppi as f64),
                };

                ui_context.executor.send_input(id, input);
            }
            event::InputEvent::MouseWheel {
                device_id,
                delta,
                phase,
            } => {
                ui_context.executor.send_input(id, ev.clone());
            }
            event::InputEvent::MouseInput {
                device_id,
                state,
                button,
            } => {
                ui_context.executor.send_input(id, ev.clone());
            }
            _ => (),
        };
        Some(())
        // if let Some(pos) = resp.interact_pointer_pos() {
        //     // let x = pos.x - resp.rect.left();
        //     // let y = pos.y - resp.rect.top();
        //     let x = resp.drag_delta().x;
        //     let y = resp.drag_delta().y;

        //     ui_context.executor.send_input(window_state.task_id, ExecutorInputEvent::MouseDrag(Vec2f::new(x, y)));
        // }
    }
    pub fn add_scene(&mut self, ui_context: &mut UIContext, scene: Scene) {
        let canvas = Canvas::new(Size::new(
            (DEFAULT_CANVAS_SIZE[0] as f32 * ui_context.ppi) as u32,
            (DEFAULT_CANVAS_SIZE[1] as f32 * ui_context.ppi) as u32,
        ));
        let id = ui_context.executor.run(0, canvas.clone(), scene);
        let texture_id = ui_context.add_canvas_and_alloc(canvas.clone());
        self.state.render_windows.insert(
            id,
            RenderWindowState {
                canvas,
                task_id: id,
                texture_id,
                name: format!("{} {}", "test", id),
                opened: true,
                closed_time: None,
                new_open: true,
                progress: 0f32,
                request_time: None,
                rect: Vec4f::default(),
            },
        );
    }
}