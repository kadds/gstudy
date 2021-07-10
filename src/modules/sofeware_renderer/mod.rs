use super::{Module, ModuleInfo};

pub struct SofewareRenderer {}

impl Module for SofewareRenderer {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            name: "sofeware renderer",
            desc: "",
        }
    }
}
