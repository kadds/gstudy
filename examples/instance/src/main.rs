use core::{
    context::RContext,
    material::{
        basic::BasicMaterialFaceBuilder, InputResource, InputResourceBuilder, MaterialBuilder,
    },
    mesh::{
        builder::{
            InstancePropertiesBuilder, InstancePropertiesUpdater, InstancePropertyType,
            INSTANCE_TRANSFORM,
        },
        InstanceProperties, StaticGeometry, TransformType,
    },
    scene::{
        controller::{orbit::OrbitCameraController, CameraController},
        Camera, RenderObject, Scene, TransformBuilder,
    },
    types::{Mat4x4f, Quaternion, Size, Vec3f, Vec4f},
    util::rad2angle,
};
use std::{
    any::Any,
    cell::RefCell,
    sync::{Arc, Mutex},
};

use app::{App, AppEventProcessor};
use geometry::mesh::*;

use window::{HardwareRenderPluginFactory, WindowPluginFactory};

#[derive(Default)]
pub struct MainLogic {
    ct: Option<Box<RefCell<dyn CameraController>>>,
    object_id: Option<u64>,
}

const X: u64 = 50;
const Z: u64 = 50;

impl MainLogic {
    fn on_startup(&mut self, scene: &core::scene::Scene) {
        let basic_material_builder = BasicMaterialFaceBuilder::new()
            .texture(InputResourceBuilder::only_instance())
            .instance();

        let material = MaterialBuilder::default()
            .face(basic_material_builder.build())
            .primitive(wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            })
            .build(&scene.context());

        {
            let mesh = Arc::new(
                CubeMeshBuilder::default()
                    // .enable_color(Color::new(0.2f32, 0.7f32, 0.7f32, 1f32))
                    .build(),
            );

            // create instance
            let mut instance_builder = InstancePropertiesBuilder::default();
            instance_builder.add_property(INSTANCE_TRANSFORM);
            let color_property = InstancePropertyType::new::<Vec4f>("color");
            instance_builder.add_property(color_property);

            for x in 0..X {
                for z in 0..Z {
                    let px = x as f32 / X as f32;
                    let pz = z as f32 / Z as f32;
                    let transform = TransformBuilder::new()
                        .translate(Vec3f::new(
                            (px - 0.5f32) * (X * 2) as f32,
                            0f32,
                            (pz - 0.5f32) * (Z * 2) as f32,
                        ))
                        .build();

                    let f = (px + pz) / 2f32;

                    instance_builder.add_property_data(INSTANCE_TRANSFORM, &[*transform.mat()]);
                    instance_builder.add_property_data(
                        color_property,
                        &[Vec4f::new(1.0f32 * px, f, 1.0f32 * pz, 1f32)],
                    );
                }
            }

            let geometry = StaticGeometry::new(mesh).with_instance(InstanceProperties {
                data: Mutex::new(instance_builder.build()),
                transform_type: TransformType::Mat4x4,
                dynamic: true,
            });

            let obj = RenderObject::new(Box::new(geometry), material.clone()).unwrap();
            self.object_id = Some(scene.add(obj));
        }

        let camera = Camera::new();
        camera.make_perspective(1f32, std::f32::consts::PI / 2f32, 0.01f32, 1000f32);

        camera.look_at(
            Vec3f::new(0f32, 4f32, 4f32),
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
            match &ev {
                core::event::Event::Update(delta) => {
                    let scene = context.container.get::<Scene>().unwrap();
                    if let Some(id) = &self.object_id {
                        let dt = *delta as f32 * 0.01f32;
                        let angle = rad2angle(dt);
                        let c = scene.get_container();
                        let obj = c.get(id).unwrap();
                        let instance = obj.object.geometry().instance().unwrap();
                        let mut data = instance.data.lock().unwrap();
                        let mut updater = InstancePropertiesUpdater::new(&mut data);

                        let mut index = 0;
                        let axis_x = Vec3f::x_axis();
                        let axis_z = Vec3f::z_axis();
                        let mut transforms =
                            updater.get_property::<Mat4x4f>(INSTANCE_TRANSFORM, index, X * Z);

                        for x in 0..X {
                            for z in 0..Z {
                                let mut raw_transform = transforms[index as usize];

                                if x % 2 == 0 {
                                    raw_transform *= Quaternion::from_axis_angle(&axis_x, angle)
                                        .to_homogeneous();
                                } else {
                                    raw_transform *= Quaternion::from_axis_angle(&axis_x, -angle)
                                        .to_homogeneous();
                                }
                                if z % 2 == 0 {
                                    raw_transform *= Quaternion::from_axis_angle(&axis_z, -angle)
                                        .to_homogeneous();
                                } else {
                                    raw_transform *= Quaternion::from_axis_angle(&axis_z, angle)
                                        .to_homogeneous();
                                }

                                transforms[index as usize] = raw_transform;
                                index += 1;
                            }
                        }

                        updater.set_property(INSTANCE_TRANSFORM, 0, &transforms);
                    }
                }
                core::event::Event::Input(input) => {
                    if let Some(ct) = &mut self.ct {
                        ct.borrow_mut().on_input(input);
                    }
                }
                _ => (),
            }
        }
    }
}

fn do_main() {
    profiling::scope!("app");
    env_logger::init();
    let context = RContext::new();

    let mut app = App::new(context);
    app.register_plugin(WindowPluginFactory::new("Instance", Size::new(900, 720)));
    app.register_plugin(HardwareRenderPluginFactory);
    app.add_event_processor(Box::new(MainLogic::default()));
    app.run();
}

fn main() {
    #[cfg(feature = "profile-with-tracy")]
    {
        let _ = profiling::tracy_client::Client::start();
        do_main();
        return;
    }

    do_main();
}
