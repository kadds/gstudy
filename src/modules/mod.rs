pub trait Module {
    fn info(&self) -> ModuleInfo;
}

pub mod ray_tracing;
pub mod sofeware_renderer;

pub struct ModuleInfo {
    name: &'static str,
    desc: &'static str,
}
