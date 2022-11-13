use winit::event_loop::EventLoopProxy;

use crate::{event::Event, render::Executor};

use super::UIContext;

pub mod entry;

pub trait Logic {
    fn update(
        &mut self,
        ctx: egui::Context,
        ui_context: &mut UIContext,
        proxy: EventLoopProxy<Event>,
    );
}

pub trait View {
    fn update(&mut self, ctx: egui::Context, ui: &mut egui::Ui);
}

pub fn init(logic: &mut Vec<Box<dyn Logic>>) {
    logic.push(Box::new(entry::EntryLogic::new()));
}
