use super::Logic;

struct EntryState {
    always_redraw: bool,
}

impl Default for EntryState {
    fn default() -> Self {
        Self {
            always_redraw: false,
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
        egui::SidePanel::left("main_side").show(&ctx, |ui| {
            ui.label("text");
        });
        egui::Window::new("settings").show(&ctx, |ui| {
            ctx.settings_ui(ui);
            ui.add(egui::Checkbox::new(
                &mut self.state.always_redraw,
                "always redraw",
            ));
        });
        egui::Window::new("style").show(&ctx, |ui| {
            ctx.style_ui(ui);
        });
        egui::Window::new("texture").show(&ctx, |ui| {
            ctx.texture_ui(ui);
        });
        egui::Window::new("inspection").show(&ctx, |ui| {
            ctx.inspection_ui(ui);
        });
        egui::Window::new("memory").show(&ctx, |ui| {
            ctx.memory_ui(ui);
        });
        if self.state.always_redraw {
            ctx.request_repaint();
        }
    }
}
