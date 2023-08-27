use core::{
    context::RContext,
    material::{basic::BasicMaterialFaceBuilder, MaterialBuilder},
    mesh::{builder::MeshBuilder, StaticGeometry},
    scene::{Camera, RenderObject, Scene},
    types::{Size, Vec3f, Vec4f},
    util::any_as_u8_slice_array,
};
use std::{any::Any, sync::Arc};

use app::{App, AppEventProcessor};
use window::{HardwareRenderPluginFactory, WindowPluginFactory};

pub struct MainLogic {}

impl MainLogic {
    fn on_startup(&mut self, scene: &core::scene::Scene) {}
}

impl AppEventProcessor for MainLogic {
    fn on_event(&mut self, context: &app::AppEventContext, event: &dyn Any) {
        if let Some(ev) = event.downcast_ref::<app::Event>() {
            match ev {
                app::Event::Startup => {
                    let scene = context.container.get::<Scene>().unwrap();
                    self.on_startup(&scene);
                }
            }
        }
    }
}

fn main() {
    env_logger::init();
    let context = RContext::new();

    let mut app = App::new(context);
    app.register_plugin(WindowPluginFactory::new("Empty", Size::new(600, 600)));
    app.register_plugin(HardwareRenderPluginFactory);
    app.add_event_processor(Box::new(MainLogic {}));
    app.run();
}
