use super::{Module, ModuleInfo};

pub struct SoftwareRenderer {}
impl SoftwareRenderer {
    pub fn new() -> Self {
        Self {}
    }
}

impl Module for SoftwareRenderer {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            name: "software renderer",
            desc: "a software renderer with shader pipeline",
        }
    }
    fn run(&mut self) {
    }

    fn stop(&mut self) {
    }

    fn pause(&mut self) {
    }

    fn resume(&mut self) {
    }
}
