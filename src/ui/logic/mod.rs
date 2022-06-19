pub mod entry;

pub trait Logic {
    fn update(&mut self, ctx: egui::Context);
}

pub fn init(logic: &mut Vec<Box<dyn Logic>>) {
    logic.push(Box::new(entry::EntryLogic::new()));
}
