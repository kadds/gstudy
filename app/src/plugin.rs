use core::{
    event::{EventProcessor, EventSender},
    render::material::MaterialRendererFactory,
    scene::controller::ControllerFactory,
};
use std::{any::TypeId, cell::RefCell, collections::HashMap, rc::Rc};

use crate::{AppEventProcessor, Container};

pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub has_looper: bool,
}

pub trait PluginFactory {
    fn create(&self, container: &Container) -> Box<dyn Plugin>;
    fn create_looper(&self, _container: &Container) -> Option<Box<dyn LooperPlugin>> {
        None
    }
    fn info(&self) -> PluginInfo;
}

#[derive(Default)]
pub struct CoreFactoryList {
    pub materials: Vec<(TypeId, Box<dyn MaterialRendererFactory>)>,
    pub camera_controllers: HashMap<String, Box<dyn ControllerFactory>>,
}

pub trait Runner: EventProcessor {
    fn startup(&self, proxy: &dyn EventSender);
}

pub trait Plugin: AppEventProcessor {
    fn load_factory(&self) -> CoreFactoryList {
        CoreFactoryList::default()
    }
    fn install_factory(&mut self, _container: &Container, _factory_list: &mut CoreFactoryList) {}
}

pub trait LooperPlugin {
    fn run(&self, _container: &Container, _runner: Rc<RefCell<dyn Runner>>) {}
}
