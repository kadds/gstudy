use std::sync::Arc;

use crate::render::{Camera, Canvas, Scene};

use super::{ModuleFactory, ModuleInfo};

pub struct RayTracingFactory {}

impl RayTracingFactory {
    pub fn new() -> Self {
        Self {}
    }
}

impl ModuleFactory for RayTracingFactory {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            name: "ray tracing",
            desc: "a ray tracing demo",
        }
    }

    fn make_renderer(&self) -> Box<dyn super::ModuleRenderer> {
        todo!();
    }
}
