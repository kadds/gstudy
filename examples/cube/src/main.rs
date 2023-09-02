use core::{
    context::RContext,
    material::{basic::BasicMaterialFaceBuilder, MaterialBuilder, MaterialMap},
    mesh::StaticGeometry,
    scene::{
        controller::{orbit::OrbitCameraController, CameraController},
        Camera, RenderObject, Scene,
    },
    types::{Color, Size, Vec3f},
};
use std::{any::Any, cell::RefCell, sync::Arc};

use app::{App, AppEventProcessor};
use geometry::cube::CubeMeshBuilder;
use window::{HardwareRenderPluginFactory, Msaa, MsaaResource, WindowPluginFactory};

#[derive(Default)]
pub struct MainLogic {
    ct: Option<Box<RefCell<dyn CameraController>>>,
}

impl MainLogic {
    fn on_startup(&mut self, scene: &core::scene::Scene) {
        let mesh = CubeMeshBuilder::default()
            .enable_color(Color::new(1.0f32, 1.0f32, 1.0f32, 1.0f32))
            .set_color_front_face(Color::new(0.8f32, 0.8f32, 0.8f32, 1f32))
            .set_color_back_face(Color::new(0.4f32, 0.9f32, 0.4f32, 1f32))
            .set_color_left_face(Color::new(0.8f32, 0.2f32, 0.2f32, 1f32))
            .set_color_right_face(Color::new(0.8f32, 0.8f32, 0.2f32, 1f32))
            .set_color_top_face(Color::new(0.2f32, 0.2f32, 0.8f32, 1f32))
            .set_color_bottom_face(Color::new(0.2f32, 0.8f32, 0.8f32, 1f32))
            .build();

        let geometry = StaticGeometry::new(Arc::new(mesh));
        let basic_material_builder =
            BasicMaterialFaceBuilder::new().texture(MaterialMap::PreVertex);
        let material = MaterialBuilder::default()
            .face(basic_material_builder.build())
            .build(&scene.context());

        let obj = RenderObject::new(Box::new(geometry), material);
        scene.add(obj);

        let camera = Camera::new();
        camera.make_perspective(1f32, std::f32::consts::PI / 2f32, 0.01f32, 100f32);

        camera.look_at(
            Vec3f::new(0f32, 0f32, 2f32),
            Vec3f::zeros(),
            Vec3f::new(0f32, 1f32, 0f32),
        );

        let camera = Arc::new(camera);

        self.ct = Some(Box::new(RefCell::new(OrbitCameraController::new(
            camera.clone(),
        ))));
        scene.set_main_camera(camera);
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
        } else if let Some(ev) = event.downcast_ref::<core::event::Event>() {
            if let core::event::Event::Input(input) = &ev {
                if let Some(ct) = &mut self.ct {
                    ct.borrow_mut().on_input(input);
                }
                match input {
                    core::event::InputEvent::KeyboardInput(key) => {
                        let sampler_count = match key.vk {
                            core::event::VirtualKeyCode::F1 => Some(1),
                            core::event::VirtualKeyCode::F2 => Some(2),
                            core::event::VirtualKeyCode::F4 => Some(4),
                            core::event::VirtualKeyCode::F8 => Some(8),
                            _ => None,
                        };
                        if let Some(s) = sampler_count {
                            context
                                .container
                                .get::<MsaaResource>()
                                .unwrap()
                                .set(Msaa(s));
                            let scene = context.container.get::<Scene>().unwrap();
                            scene.set_rebuild_flag();
                        }
                    }
                    _ => (),
                }
            }
        }
    }
}

fn main() {
    env_logger::init();
    let context = RContext::new();

    let mut app = App::new(context);
    app.register_plugin(WindowPluginFactory::new(
        "Cube Msaa-F1-F2-F4-F8",
        Size::new(600, 600),
    ));
    app.register_plugin(HardwareRenderPluginFactory);
    app.add_event_processor(Box::new(MainLogic::default()));
    app.run();
}
