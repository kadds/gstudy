use super::Logic;

struct EntryState {
    always_redraw: bool,
    show_settings: bool,
    show_style: bool,
    show_texture: bool,
    show_inspection: bool,
    show_memory: bool,
    show_about: bool,
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
        }
    }
}

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
    fn update(&mut self, ctx: egui::Context) {
        let state = &mut self.state;
        egui::SidePanel::left("main_side").show(&ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {});
                ui.menu_button("Setting", |ui| {
                    ui.checkbox(&mut state.always_redraw, "Always_redraw");
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
            ui.heading("Functions");
            ui.separator();
        });
        egui::Window::new("Settings ui")
            .open(&mut state.show_settings)
            .show(&ctx, |ui| {
                ctx.settings_ui(ui);
            });
        egui::Window::new("Style ui")
            .open(&mut state.show_style)
            .show(&ctx, |ui| {
                ctx.style_ui(ui);
            });
        egui::Window::new("Texture ui")
            .open(&mut state.show_texture)
            .show(&ctx, |ui| {
                ctx.texture_ui(ui);
            });
        egui::Window::new("Inspection ui")
            .open(&mut state.show_inspection)
            .show(&ctx, |ui| {
                ctx.inspection_ui(ui);
            });
        egui::Window::new("Memory ui")
            .open(&mut state.show_memory)
            .show(&ctx, |ui| {
                ctx.memory_ui(ui);
            });
        egui::Window::new("About")
            .vscroll(true)
            .collapsible(false)
            .fixed_size(&[500f32, 260f32])
            .anchor(egui::Align2::CENTER_CENTER, &[0f32, 0f32])
            .open(&mut state.show_about)
            .show(&ctx, |ui| {
                use egui::special_emojis::*;
                ui.label(egui::RichText::new("GStudy project").heading().strong());
                ui.label(
                    egui::RichText::new(format!(
                        "build info: {} {}, \ncommit {} at {}",
                        env!("VERGEN_CARGO_PROFILE"),
                        env!("VERGEN_CARGO_TARGET_TRIPLE"),
                        env!("VERGEN_BUILD_DATE"),
                        env!("VERGEN_GIT_SHA_SHORT")
                    ))
                );
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
