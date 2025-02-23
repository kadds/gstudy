use core::{
    context::RContext,
    material::{
        basic::BasicMaterialFaceBuilder, input::InputResourceBuilder, MaterialBuilder
    },
    mesh::{
        builder::{MeshBuilder, MeshPropertiesBuilder, MeshPropertyType},
        StaticGeometry,
    },
    scene::{Camera, RenderObject, Scene},
    types::{Color, Size, Vec3f, Vec4f},
};
use std::{any::Any, sync::Arc};

use app::{App, AppEventProcessor};
use window::{HardwareRenderPluginFactory, WindowPluginFactory};

pub struct MainLogic {}

impl MainLogic {
    fn on_startup(&mut self, scene: &core::scene::Scene) {
        let mut builder = MeshBuilder::default();
        let mut properties_builder = MeshPropertiesBuilder::default();
        let property = MeshPropertyType::new::<Color>("color");
        properties_builder.add_property(property);

        builder.add_position_vertices3(&[
            Vec3f::new(0f32, -0.5f32, 0f32),
            Vec3f::new(-0.7f32, 0.7f32, 0f32),
            Vec3f::new(0.7f32, 0.7f32, 0f32),
        ]);
        builder.add_indices32(&[0, 1, 2]);
        properties_builder.add_property_data(
            property,
            &[
                Vec4f::new(1f32, 0f32, 0f32, 1f32),
                Vec4f::new(0f32, 1f32, 0f32, 1f32),
                Vec4f::new(0f32, 0f32, 1f32, 1f32),
            ],
        );
        builder.set_properties(properties_builder.build());

        let mesh = builder.build().unwrap();

        let geometry = StaticGeometry::new(Arc::new(mesh));
        let basic_material_builder =
            BasicMaterialFaceBuilder::new().texture(InputResourceBuilder::only_pre_vertex());

        let material = MaterialBuilder::default()
            .face(basic_material_builder.build())
            .build(&scene.context());

        let obj = RenderObject::new(Box::new(geometry), material).unwrap();
        scene.add(obj);

        let camera = Camera::new();
        camera.make_orthographic(Vec4f::new(-1f32, 1f32, 1f32, -1f32), 0.1f32, 2.0f32);

        camera.look_at(
            Vec3f::new(0f32, 0f32, 1f32),
            Vec3f::zeros(),
            Vec3f::new(0f32, 1f32, 0f32),
        );

        scene.set_main_camera(Arc::new(camera));
    }
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
    app.register_plugin(WindowPluginFactory::new("Triangle", Size::new(600, 600)));
    app.register_plugin(HardwareRenderPluginFactory);
    app.add_event_processor(Box::new(MainLogic {}));
    app.run();
}
