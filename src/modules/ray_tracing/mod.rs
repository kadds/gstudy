use super::{Module, ModuleInfo};

pub struct RayTracing {}

impl Module for RayTracing {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            name: "ray tracing",
            desc: "",
        }
    }
}
