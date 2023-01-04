use std::sync::Arc;

use crate::{
    core::backends::wgpu_backend::WGPUResource,
    render::{Camera, Scene},
};

pub struct RenderParameter<'a> {
    pub gpu: Arc<WGPUResource>,
    pub scene: &'a mut Scene,
}

pub trait ModuleRenderer {
    fn render(&mut self, parameter: RenderParameter);
    fn stop(&mut self);
}

pub trait ModuleFactory: Sync + Send {
    fn info(&self) -> ModuleInfo;
    fn make_renderer(&self) -> Box<dyn ModuleRenderer>;
}

pub mod hardware_renderer;
pub use hardware_renderer::HardwareRendererFactory;

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: &'static str,
    pub desc: &'static str,
}
