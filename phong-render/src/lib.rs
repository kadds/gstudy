use core::scene::Scene;
use std::any::TypeId;

use app::{
    container::Container,
    plugin::{CoreFactoryList, Plugin, PluginFactory},
    AppEventProcessor,
};
use light::SceneLights;
use material::PhongMaterialFace;
use material_render::PhongMaterialRendererFactory;

pub mod light;
pub mod material;
pub mod material_render;

#[derive(Default)]
pub struct PhongPluginFactory {}

impl PluginFactory for PhongPluginFactory {
    fn create(&self, container: &app::container::Container) -> Box<dyn app::plugin::Plugin> {
        Box::new(PhongPlugin::new(container))
    }

    fn info(&self) -> app::plugin::PluginInfo {
        app::plugin::PluginInfo {
            name: "phong".into(),
            version: "0.1.0".into(),
            has_looper: false,
        }
    }
}

pub struct PhongPlugin {}

impl PhongPlugin {
    pub fn new(container: &Container) -> Self {
        Self {}
    }
}

impl Plugin for PhongPlugin {
    fn load_factory(&self) -> app::plugin::CoreFactoryList {
        CoreFactoryList {
            materials: vec![(
                TypeId::of::<PhongMaterialFace>(),
                Box::new(PhongMaterialRendererFactory {}),
            )],
            ..Default::default()
        }
    }
}

impl AppEventProcessor for PhongPlugin {
    fn on_event(&mut self, context: &app::AppEventContext, event: &dyn std::any::Any) {
        if let Some(ev) = event.downcast_ref::<core::event::Event>() {}
    }
}
