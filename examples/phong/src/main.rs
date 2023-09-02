use core::{
    context::RContext,
    material::{MaterialBuilder, MaterialMap},
    mesh::StaticGeometry,
    scene::{
        controller::{orbit::OrbitCameraController, CameraController},
        Camera, RenderObject, Scene, TransformBuilder,
    },
    types::{Color, Size, Vec3f},
};
use std::{any::Any, cell::RefCell, sync::Arc};

use app::{App, AppEventProcessor};
use geometry::{cube::CubeMeshBuilder, plane::PlaneMeshBuilder};
use phong_render::{
    light::{DirectLightBuilder, SceneLights},
    material::PhongMaterialFaceBuilder,
    PhongPluginFactory,
};
use window::{HardwareRenderPluginFactory, WindowPluginFactory};

#[derive(Default)]
pub struct MainLogic {
    ct: Option<Box<RefCell<dyn CameraController>>>,
}

impl MainLogic {
    fn on_startup(&mut self, scene: &core::scene::Scene, lights: &SceneLights) {
        let basic_material_builder = PhongMaterialFaceBuilder::new()
            .diffuse(MaterialMap::PreVertex)
            .normal(MaterialMap::PreVertex)
            .specular(MaterialMap::Constant(Color::new(
                0.8f32, 0.8f32, 0.8f32, 1f32,
            )))
            .shininess(4f32);

        let material = MaterialBuilder::default()
            .face(basic_material_builder.build())
            .build(&scene.context());

        {
            let mesh = CubeMeshBuilder::default()
                .enable_normal()
                .enable_color(Color::new(0.7f32, 0.2f32, 0.2f32, 1f32))
                .build();

            let geometry = StaticGeometry::new(Arc::new(mesh)).with_transform(
                TransformBuilder::new()
                    .translate(Vec3f::new(0f32, 0.5001f32, 0f32))
                    .build(),
            );
            let obj = RenderObject::new(Box::new(geometry), material.clone());
            scene.add(obj);
        }

        {
            let mesh = PlaneMeshBuilder::default()
                .enable_normal()
                .enable_color(Color::new(0.2f32, 0.2f32, 0.22f32, 1f32))
                .build();

            let geometry = StaticGeometry::new(Arc::new(mesh)).with_transform(
                TransformBuilder::new()
                    .scale(Vec3f::new(100f32, 1f32, 100f32))
                    .build(),
            );
            let obj = RenderObject::new(Box::new(geometry), material.clone());
            scene.add(obj);
        }

        let camera = Camera::new();
        camera.make_perspective(1f32, std::f32::consts::PI / 2f32, 0.01f32, 100f32);

        camera.look_at(
            Vec3f::new(2f32, 2f32, 2f32),
            Vec3f::zeros(),
            Vec3f::new(0f32, 1f32, 0f32),
        );

        let camera = Arc::new(camera);

        self.ct = Some(Box::new(RefCell::new(OrbitCameraController::new(
            camera.clone(),
        ))));
        scene.set_main_camera(camera);

        let light = DirectLightBuilder::new()
            .position(Vec3f::new(5f32, 5f32, 5f32))
            .direction(Vec3f::new(-2f32, -2f32, -1f32))
            .color(Color::new(0.7f32, 0.7f32, 0.62f32, 1f32))
            .cast_shadow(true)
            .build();
        lights.set_direct_light(light);
        lights.set_ambient(Color::new(0.2f32, 0.2f32, 0.2f32, 1.0f32));
        scene.set_rebuild_flag();
    }
}

impl AppEventProcessor for MainLogic {
    fn on_event(&mut self, context: &app::AppEventContext, event: &dyn Any) {
        if let Some(ev) = event.downcast_ref::<app::Event>() {
            match ev {
                app::Event::Startup => {
                    let lights = context.container.get::<SceneLights>().unwrap();
                    let scene = context.container.get::<Scene>().unwrap();
                    self.on_startup(&scene, &lights);
                }
            }
        } else if let Some(ev) = event.downcast_ref::<core::event::Event>() {
            if let core::event::Event::Input(input) = &ev {
                if let Some(ct) = &mut self.ct {
                    ct.borrow_mut().on_input(input);
                }
            }
        }
    }
}

fn main() {
    env_logger::init();
    let context = RContext::new();

    let mut app = App::new(context);
    app.register_plugin(WindowPluginFactory::new("Phong", Size::new(800, 600)));
    app.register_plugin(HardwareRenderPluginFactory);
    app.register_plugin(PhongPluginFactory {});
    app.add_event_processor(Box::new(MainLogic::default()));
    app.run();
}
