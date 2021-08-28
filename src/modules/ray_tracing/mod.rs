use std::thread::{self, Thread};

use super::{Module, ModuleInfo};

pub struct RayTracing {}

impl RayTracing {
    pub fn new() -> Self {
        Self {}
    }
}

impl Module for RayTracing {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            name: "ray tracing",
            desc: "a ray tracing demo",
        }
    }

    fn run(&mut self) {}

    fn stop(&mut self) {}

    fn pause(&mut self) {}

    fn resume(&mut self) {}
}
