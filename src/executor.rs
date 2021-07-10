use crate::modules::*;

pub struct Executor {
    modules: Vec<Box<dyn Module>>,
    current_module: Option<usize>,
}

impl Executor {
    pub fn new() -> Self {
        let mut modules: Vec<Box<dyn Module>> = Vec::new();
        modules.push(Box::new(RayTracing::new()));
        modules.push(Box::new(SoftwareRenderer::new()));
        Self {
            modules,
            current_module: None,
        }
    }

    pub fn run(&mut self, name: &str) {
        let module_index = self.match_module(name);
        if let Some(_) = self.current_module {
            let m = self.current_module();
            m.stop();
        }
        self.current_module = Some(module_index);
        let m = self.current_module();
        m.run();
    }

    fn current_module(&mut self) -> &mut dyn Module {
        let e = &mut self.modules[self.current_module.unwrap()];
        e.as_mut()
    }

    pub fn stop(&mut self) {
        let m = self.current_module();
        m.stop();
    }

    pub fn pause(&mut self) {
        let m = self.current_module();
        m.pause();
    }

    pub fn resume(&mut self) {
        let m = self.current_module();
        m.resume();
    }

    pub fn restart(&mut self) {
        let m = self.current_module();
        m.stop();
        m.run();
    }

    fn match_module(&self, name: &str) -> usize {
        self.modules
            .iter()
            .enumerate()
            .find(|it| it.1.info().name == name)
            .unwrap()
            .0
    }

    pub fn list(&self) -> Vec<ModuleInfo> {
        self.modules.iter().map(|it| it.info()).collect()
    }
}
