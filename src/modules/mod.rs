use std::sync::Arc;

use crate::render::{Camera, Canvas, Scene};
use crate::renderer::GpuContextRc;

pub struct RenderParameter<'a> {
    pub gpu_context: GpuContextRc,
    pub camera: &'a Camera,
    pub scene: &'a Scene,
    pub canvas: &'a Canvas,
}

pub trait ModuleRenderer: Send {
    fn render(&mut self, parameter: RenderParameter);
    fn stop(&mut self);
}

pub trait ModuleFactory: Sync + Send {
    fn info(&self) -> ModuleInfo;
    fn make_renderer(&self) -> Box<dyn ModuleRenderer>;
}

pub mod hardware_renderer;
pub mod ray_tracing;
pub mod software_renderer;
pub use hardware_renderer::HardwareRendererFactory;
pub use ray_tracing::RayTracingFactory;
pub use software_renderer::SoftwareRendererFactory;

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: &'static str,
    pub desc: &'static str,
}
