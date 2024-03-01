use core::{
    context::{RContext, TagId},
    debug::{new_debug_material, DebugMeshGenerator},
    material::{InputResource, InputResourceBuilder, Material, MaterialBuilder},
    mesh::{DynamicGeometry, StaticGeometry},
    scene::{
        controller::{orbit::OrbitCameraController, CameraController},
        Camera, ObjectId, RenderObject, Scene, TransformBuilder, LAYER_NORMAL,
    },
    types::{Color, Size, Vec3f},
    util::angle2rad,
};
use std::{any::Any, cell::RefCell, sync::Arc};

use app::{App, AppEventProcessor};
use geometry::{mesh::CubeMeshBuilder, mesh::PlaneMeshBuilder, mesh::UVSphereBuilder};
use phong_render::{
    light::{
        DirectLightBuilder, PointLightBuilder, SceneLights, ShadowConfig, SpotLightBuilder, TLight,
    },
    material::PhongMaterialFaceBuilder,
    PhongPluginFactory,
};
use window::{HardwareRenderPluginFactory, WindowPluginFactory};

#[derive(Default)]
pub struct MainLogic {
    ct: Option<Box<RefCell<dyn CameraController>>>,
    debug_materia: Option<Arc<Material>>,
    debug_tag: Option<TagId>,
    id: ObjectId,
}

impl MainLogic {
    fn update(&mut self, delta: f32, scene: &core::scene::Scene) {
        let lights = scene.get_resource::<SceneLights>().unwrap();
        let mut meshes = vec![];

        if lights.has_direct_light() {
            let dlight = lights.direct_light().unwrap();
            meshes.push(
                dlight.light_cameras()[0]
                    .frustum_worldspace()
                    .generate(Color::new(0.8f32, 0.92f32, 0.84f32, 1.0f32)),
            );
        };

        for light in &lights.extra_lights() {
            let color = match light.as_ref() {
                phong_render::light::Light::Spot(_) => Color::new(0.9f32, 0.84f32, 0.77f32, 1f32),
                // phong_render::light::Light::Point(_) => Color::new(0.72f32, 0.84f32, 0.97f32, 1f32),
                _ => {
                    continue;
                }
            };

            meshes.extend_from_slice(
                &light
                    .light_cameras()
                    .iter()
                    .map(|c| c.frustum_worldspace().generate(color))
                    .collect::<Vec<_>>(),
            );
        }
        let core_mesh = core::mesh::merge::MeshMerger::merge_all(meshes.into_iter()).unwrap();

        let container = scene.get_container();
        let obj = container.get(&self.id).unwrap();
        obj.object.geometry().update_mesh(Arc::new(core_mesh));
    }

    fn on_startup(&mut self, scene: &core::scene::Scene) {
        self.debug_materia = Some(new_debug_material(&scene.context()));
        self.debug_tag = Some(scene.context().new_tag("debug_mesh"));

        let lights = SceneLights::default();

        let basic_material_builder = PhongMaterialFaceBuilder::new()
            .diffuse(InputResourceBuilder::only_pre_vertex())
            .normal(InputResourceBuilder::only_pre_vertex())
            .specular(InputResourceBuilder::only_constant(Color::new(
                0.7f32, 0.7f32, 0.7f32, 1f32,
            )))
            .shininess(4f32);

        let material = MaterialBuilder::default()
            .face(basic_material_builder.build())
            .build(&scene.context());

        let basic_material_builder2 = PhongMaterialFaceBuilder::new()
            .diffuse(InputResourceBuilder::only_pre_vertex())
            .normal(InputResourceBuilder::only_pre_vertex())
            .emissive(InputResourceBuilder::only_constant(Color::new(
                0.2f32, 0.2f32, 0.1f32, 1f32,
            )))
            .shininess(32f32)
            .specular(InputResourceBuilder::only_constant(Color::new(
                0.7f32, 0.7f32, 0.7f32, 1f32,
            )))
            .shininess(4f32);

        let material2 = MaterialBuilder::default()
            .face(basic_material_builder2.build())
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
            let mut obj = RenderObject::new(Box::new(geometry), material.clone()).unwrap();
            obj.set_name("cube0");
            scene.add(obj);
        }

        {
            let mesh = CubeMeshBuilder::default()
                .enable_normal()
                .enable_color(Color::new(0.4f32, 0.8f32, 0.4f32, 1f32))
                .build();

            let geometry = StaticGeometry::new(Arc::new(mesh)).with_transform(
                TransformBuilder::new()
                    .translate(Vec3f::new(0f32, 1.2501f32, 0f32))
                    .scale(Vec3f::new(0.5f32, 0.5f32, 0.5f32))
                    .build(),
            );
            let mut obj = RenderObject::new(Box::new(geometry), material.clone()).unwrap();
            obj.set_name("cube");
            scene.add(obj);
        }

        {
            let mesh = UVSphereBuilder::default()
                .enable_normal()
                .set_segments(32, 24)
                .enable_color(Color::new(0.6f32, 0.7f32, 0.8f32, 1f32))
                .build();

            let geometry = StaticGeometry::new(Arc::new(mesh)).with_transform(
                TransformBuilder::new()
                    .translate(Vec3f::new(1.2f32, 0.501f32, -0.2f32))
                    .build(),
            );
            let mut obj = RenderObject::new(Box::new(geometry), material.clone()).unwrap();
            obj.set_name("sphere");
            scene.add(obj);
        }

        {
            let mesh = UVSphereBuilder::default()
                .enable_normal()
                .set_segments(32, 24)
                .enable_color(Color::new(0.6f32, 0.7f32, 0.8f32, 1f32))
                .build();

            let geometry = StaticGeometry::new(Arc::new(mesh)).with_transform(
                TransformBuilder::new()
                    .translate(Vec3f::new(-1.5f32, 0.501f32, -0.8f32))
                    .build(),
            );
            let mut obj = RenderObject::new(Box::new(geometry), material2.clone()).unwrap();
            obj.set_name("sphere1");
            scene.add(obj);
        }

        {
            let mesh = PlaneMeshBuilder::default()
                .enable_normal()
                .enable_color(Color::new(0.2f32, 0.2f32, 0.22f32, 1f32))
                .build();

            let geometry = StaticGeometry::new(Arc::new(mesh)).with_transform(
                TransformBuilder::new()
                    .scale(Vec3f::new(20f32, 1f32, 20f32))
                    .build(),
            );
            let mut obj = RenderObject::new(Box::new(geometry), material.clone()).unwrap();
            obj.set_name("plane");
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
            .position(Vec3f::new(5f32, 5.8f32, 5f32))
            .direction(Vec3f::new(-0f32, -2f32, -0f32))
            .color(Color::new(0.5f32, 0.5f32, 0.5f32, 1f32))
            .intensity(0.8f32)
            .cast_shadow(ShadowConfig {
                cast_shadow: true,
                pcf: true,
                bias_factor: 0.5f32,
                ..Default::default()
            })
            .build();
        lights.set_direct_light(light);
        // lights.set_ambient(Color::new(0.0f32, 0.0f32, 0.0f32, 1.0f32));
        lights.set_ambient(Color::new(0.2f32, 0.2f32, 0.2f32, 1.0f32));

        let point_light = PointLightBuilder::new()
            .position(Vec3f::new(2f32, 4f32, -4f32))
            .color(Color::new(0.67f32, 0.52f32, 0.51f32, 1f32))
            .intensity(0.8f32)
            .cast_shadow(ShadowConfig {
                cast_shadow: true,
                pcf: true,
                bias_factor: 0.02f32,
                ..Default::default()
            })
            .build();

        lights.add_point_light(point_light);

        let spot_light = SpotLightBuilder::new()
            .position(Vec3f::new(-4f32, 4f32, -4.1f32))
            .direction(Vec3f::new(0.2f32, -0.3f32, 0.2f32))
            .cutoff(angle2rad(20f32), angle2rad(28f32))
            .color(Color::new(0.51f32, 0.44f32, 0.7f32, 1f32))
            .cast_shadow(ShadowConfig {
                cast_shadow: true,
                pcf: true,
                bias_factor: 0.02f32,
                ..Default::default()
            })
            .build();

        lights.add_spot_light(spot_light);

        scene.attach(Arc::new(lights));
        scene.set_rebuild_flag();

        self.id = scene.add_with_tag(
            RenderObject::new(
                Box::new(DynamicGeometry::new_empty()),
                self.debug_materia.clone().unwrap(),
            )
            .unwrap(),
            LAYER_NORMAL,
            self.debug_tag.unwrap(),
        );
        log::info!("startup {}", self.id);
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
            match ev {
                core::event::Event::Update(delta) => {
                    let scene = context.container.get::<Scene>().unwrap();
                    self.update(*delta as f32, &scene);
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
    env_logger::init();
    let context = RContext::new();

    let mut app = App::new(context);
    app.register_plugin(WindowPluginFactory::new("Phong", Size::new(800, 600)));
    app.register_plugin(HardwareRenderPluginFactory);
    app.register_plugin(PhongPluginFactory {});
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
