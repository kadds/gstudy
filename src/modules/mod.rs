pub trait Module {
    fn info(&self) -> ModuleInfo;
    fn run(&mut self);
    fn stop(&mut self);
    fn pause(&mut self);
    fn resume(&mut self);
}

pub mod ray_tracing;
pub mod software_renderer;
pub use ray_tracing::RayTracing;
pub use software_renderer::SoftwareRenderer;

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: &'static str,
    pub desc: &'static str,
}
