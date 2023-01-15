pub enum ResourceStatus {
    Unloaded,
    Loading,
    Loaded,
    Installing,
    Installed,
}

pub struct ResourceRegistry {}

pub trait ResourceFactory {
    fn load(&self, name: &str);
    fn install(&self);
}

pub struct ResourceDefines {
    ident: String,
    dependencies: Vec<String>,
}

impl ResourceRegistry {
    pub fn new() -> Self {
        todo!()
    }
}
