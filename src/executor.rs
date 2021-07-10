use std::collections::HashMap;

use crate::modules::{ray_tracing::RayTracing, Module, ModuleInfo};

pub struct Executor {
    modules: Vec<(&'static str, Box<dyn Module>)>,
    current_module: Option<&'static Module>,
}

impl Executor {
    pub fn new() -> Self {
        let modules = vec![[RayTracing::new()]];
        Self {
            modules: modules,
            current_module: None,
        }
    }

    pub fn run(&mut self, name: &str) {}

    pub fn stop(&mut self) {}

    pub fn pause(&mut self) {}

    pub fn resume(&mut self) {}

    pub fn restart(&mut self) {}

    fn match_module(&self, name: &str) -> &dyn Module {
        self.modules
            .iter()
            .find(|(n, _)| *n == name)
            .unwrap()
            .1
            .as_ref()
    }

    pub fn list(&self) -> Vec<ModuleInfo> {
        todo!();
    }
}
