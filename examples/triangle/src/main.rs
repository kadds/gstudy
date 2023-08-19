use core::{
    context::RContext,
    geometry::{MeshBuilder, StaticGeometry},
    material::{basic::BasicMaterialFaceBuilder, MaterialBuilder},
    scene::{Camera, RenderObject, Scene},
    types::{Size, Vec3f, Vec4f},
    util::any_as_u8_slice_array,
};
use std::{any::Any, sync::Arc};

use app::{App, AppEventProcessor};
use window::{HardwareRenderPluginFactory, WindowPluginFactory};

pub struct MainLogic {}

impl MainLogic {
    fn on_startup(&mut self, scene: &core::scene::Scene) {
        let mut builder = MeshBuilder::new();
        builder.add_props(core::geometry::MeshCoordType::Color);
        let mut data_builder = builder.finish_props();
        data_builder.add_vertices_position(&[
            Vec3f::new(0f32, -0.5f32, 0f32),
            Vec3f::new(-0.7f32, 0.7f32, 0f32),
            Vec3f::new(0.7f32, 0.7f32, 0f32),
        ]);
        data_builder.add_indices(&[0, 1, 2]);
        data_builder.add_vertices_prop(
            core::geometry::MeshCoordType::Color,
            any_as_u8_slice_array(&[
                Vec4f::new(1f32, 0f32, 0f32, 1f32),
                Vec4f::new(0f32, 1f32, 0f32, 1f32),
                Vec4f::new(0f32, 0f32, 1f32, 1f32),
            ]),
            16,
        );

        let mesh = data_builder.build();

        let geometry = StaticGeometry::new(Arc::new(mesh));
        let basic_material_builder = BasicMaterialFaceBuilder::new().with_color();
        let material = MaterialBuilder::default()
            .with_face(basic_material_builder.build())
            .build(&scene.context());

        let obj = RenderObject::new(Box::new(geometry), material);
        scene.add(obj);

        let camera = Camera::new();
        camera.make_orthographic(Vec4f::new(1f32, -1f32, -1f32, 1f32), 0.1f32, 7f32);
        // camera.make_perspective(1f32, std::f32::consts::PI / 2f32, 0.01f32, 100f32);

        camera.look_at(
            Vec3f::new(0f32, 0f32, 5f32),
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
