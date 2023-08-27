use core::{
    context::RContext,
    material::{basic::BasicMaterialFaceBuilder, MaterialBuilder, MaterialMap},
    mesh::{builder::MeshBuilder, StaticGeometry},
    scene::{
        controller::{orbit::OrbitCameraController, CameraController},
        Camera, RenderObject, Scene,
    },
    types::{Size, Vec3f, Vec4f},
};
use std::{any::Any, cell::RefCell, sync::Arc};

use app::{App, AppEventProcessor};
use window::{HardwareRenderPluginFactory, Msaa, MsaaResource, WindowPluginFactory};

#[derive(Default)]
pub struct MainLogic {
    ct: Option<Box<RefCell<dyn CameraController>>>,
}

impl MainLogic {
    fn on_startup(&mut self, scene: &core::scene::Scene) {
        let mut builder = MeshBuilder::new();
        builder.add_property(core::mesh::MeshPropertyType::Color);

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
        builder.add_property_vertices(
            core::mesh::MeshPropertyType::Color,
            &[
                Vec4f::new(1f32, 0f32, 0f32, 1f32),
                Vec4f::new(0f32, 0f32, 0f32, 1f32),
                Vec4f::new(0f32, 1f32, 0f32, 1f32),
                Vec4f::new(0f32, 0f32, 1f32, 1f32),
                Vec4f::new(1f32, 0f32, 1f32, 1f32),
                Vec4f::new(0f32, 1f32, 1f32, 1f32),
                Vec4f::new(1f32, 1f32, 1f32, 1f32),
                Vec4f::new(1f32, 1f32, 0f32, 1f32),
            ],
        );

        let mesh = builder.build().unwrap();

        let geometry = StaticGeometry::new(Arc::new(mesh));
        let basic_material_builder =
            BasicMaterialFaceBuilder::new().texture(MaterialMap::PreVertex);
        let material = MaterialBuilder::default()
            .face(basic_material_builder.build())
            .primitive(wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            })
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
