use core::{
    context::RContext,
    material::{MaterialBuilder, MaterialMap},
    mesh::{builder::MeshBuilder, StaticGeometry},
    scene::{
        controller::{orbit::OrbitCameraController, CameraController},
        Camera, RenderObject, Scene,
    },
    types::{Color, Size, Vec3f, Vec4f},
};
use std::{any::Any, cell::RefCell, sync::Arc};

use app::{App, AppEventProcessor};
use phong_render::{
    light::{DirectLightBuilder, SceneLights},
    material::PhongMaterialFaceBuilder,
    PhongPluginFactory,
};
use window::{HardwareRenderPluginFactory, Msaa, MsaaResource, WindowPluginFactory};

#[derive(Default)]
pub struct MainLogic {
    ct: Option<Box<RefCell<dyn CameraController>>>,
}

impl MainLogic {
    fn on_startup(&mut self, scene: &core::scene::Scene, lights: &SceneLights) {
        let mut builder = MeshBuilder::new();

        builder.add_position_vertices3(&[
            Vec3f::new(1f32, -1f32, -1f32),
            Vec3f::new(1f32, 1f32, -1f32),
            Vec3f::new(1f32, 1f32, 1f32),
            Vec3f::new(1f32, -1f32, 1f32),
            Vec3f::new(-1f32, -1f32, 1f32),
            Vec3f::new(-1f32, 1f32, 1f32),
            Vec3f::new(-1f32, 1f32, -1f32),
            Vec3f::new(-1f32, -1f32, -1f32),
        ]);
        builder.add_indices32(&[
            0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4, 0, 6, 1, 6, 0, 7, 2, 5, 3, 5, 4, 3, 1, 6, 2, 2, 6,
            5, 0, 3, 4, 4, 7, 0,
        ]);

        let mesh = builder.build().unwrap();

        let geometry = StaticGeometry::new(Arc::new(mesh));
        let basic_material_builder = PhongMaterialFaceBuilder::new().diffuse(
            MaterialMap::Constant(Color::new(0.8f32, 0.2f32, 0.1f32, 1f32)),
        );

        let material = MaterialBuilder::default()
            .face(basic_material_builder.build())
            .build(&scene.context());

        let obj = RenderObject::new(Box::new(geometry), material);
        scene.add(obj);

        let camera = Camera::new();
        camera.make_perspective(1f32, std::f32::consts::PI / 2f32, 0.01f32, 100f32);

        camera.look_at(
            Vec3f::new(0f32, 0f32, 5f32),
            Vec3f::zeros(),
            Vec3f::new(0f32, 1f32, 0f32),
        );

        let camera = Arc::new(camera);

        self.ct = Some(Box::new(RefCell::new(OrbitCameraController::new(
            camera.clone(),
        ))));
        scene.set_main_camera(camera);

        let light = DirectLightBuilder::new()
            .position(Vec3f::new(10f32, 10f32, 10f32))
            .direction(Vec3f::new(-1f32, -1f32, -1f32))
            .build();
        lights.set_direct_light(light);
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
