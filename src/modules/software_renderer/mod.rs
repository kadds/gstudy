use std::sync::Arc;

use crate::render::{Camera, Canvas, Scene};

use super::{ModuleFactory, ModuleInfo};

pub struct SoftwareRendererFactory {}
impl SoftwareRendererFactory {
    pub fn new() -> Self {
        Self {}
    }
}

impl ModuleFactory for SoftwareRendererFactory {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            name: "software renderer",
            desc: "a software renderer with shader pipeline",
        }
    }
    fn make_renderer(&self) -> Box<dyn super::ModuleRenderer> {
        todo!();
    }
}
