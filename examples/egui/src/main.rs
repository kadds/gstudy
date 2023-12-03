use core::{context::RContext, types::Size};
use std::any::Any;

use app::{App, AppEventProcessor};
use egui_render::EguiPluginFactory;
use egui_render::egui;
use window::{HardwareRenderPluginFactory, WindowPluginFactory};

pub struct MainLogic {}

impl AppEventProcessor for MainLogic {
    fn on_event(&mut self, context: &app::AppEventContext, event: &dyn Any) {
        if let Some(ev) = event.downcast_ref::<core::event::Event>() {
            match ev {
                core::event::Event::Update(_) => {
                    let ctx = context.container.get::<egui::Context>().unwrap();
                    draw_egui(&ctx);
                }
                _ => (),
            }
        }
    }
}

fn draw_egui(ctx: &egui::Context) {
    egui::Window::new("Setting")
        .resizable(true)
        .hscroll(true)
        .show(ctx, |ui| ctx.settings_ui(ui));
    egui::Window::new("Memory").show(ctx, |ui| ctx.memory_ui(ui));
    egui::Window::new("Style").show(ctx, |ui| ctx.style_ui(ui));
    egui::Window::new("Texture").show(ctx, |ui| ctx.texture_ui(ui));
    egui::Window::new("Inspection").show(ctx, |ui| ctx.inspection_ui(ui));
}

fn do_main() {
    profiling::scope!("app");
    env_logger::init();
    let context = RContext::new();

    let mut app = App::new(context);
    app.register_plugin(WindowPluginFactory::new("egui", Size::new(1300, 900)));
    app.register_plugin(HardwareRenderPluginFactory);
    app.register_plugin(EguiPluginFactory {});
    app.add_event_processor(Box::new(MainLogic {}));
    app.run();
}

fn main() {
    #[cfg(feature = "profile-with-tracy")]
    {
        let _ = profiling::tracy_client::Client::start();
        do_main();
        return;
    }

    do_main();
}
