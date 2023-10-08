use core::{
    context::RContext,
    material::{basic::BasicMaterialFaceBuilder, InputResourceBuilder, MaterialBuilder},
    mesh::StaticGeometry,
    scene::{Camera, RenderObject, Scene, TransformBuilder, LAYER_NORMAL},
    types::{Color, Size, Vec3f},
};
use std::{any::Any, sync::Arc};

use app::{App, AppEventProcessor};
use egui_render::EguiPluginFactory;
use geometry::mesh::*;
use window::{HardwareRenderPluginFactory, WindowPluginFactory};

#[derive(Default)]
pub struct MainLogic {}

impl MainLogic {
    fn on_startup(&mut self, scene: &core::scene::Scene) {
        let basic_material_builder =
            BasicMaterialFaceBuilder::new().texture(InputResourceBuilder::only_pre_vertex());
        let material = MaterialBuilder::default()
            .face(basic_material_builder.build())
            .primitive(wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            })
            .build(&scene.context());

        {
            let mesh = CubeMeshBuilder::default()
                .enable_color(Color::new(1.0f32, 0.2f32, 0.2f32, 1.0f32))
                .build();

            let geometry = StaticGeometry::new(Arc::new(mesh)).with_transform(
                TransformBuilder::new()
                    .translate(Vec3f::new(1f32, 0.5f32, 1f32))
                    .build(),
            );

            let obj = RenderObject::new(Box::new(geometry), material.clone()).unwrap();
            scene.add(obj);
        }

        {
            let mesh = PlaneMeshBuilder::default()
                .enable_color(Color::new(0.8f32, 0.8f32, 0.8f32, 1.0f32))
                .set_segments(4, 4)
                .build();

            let geometry = StaticGeometry::new(Arc::new(mesh)).with_transform(
                TransformBuilder::new()
                    .scale(Vec3f::new(20f32, 1f32, 20f32))
                    .build(),
            );

            let obj = RenderObject::new(Box::new(geometry), material.clone()).unwrap();
            scene.add_with(obj, LAYER_NORMAL + 1);
        }

        {
            let mesh = UVSphereBuilder::default()
                .enable_color(Color::new(0.2f32, 0.7f32, 0.7f32, 1f32))
                .set_segments(48, 32)
                .build();

            let geometry = StaticGeometry::new(Arc::new(mesh)).with_transform(
                TransformBuilder::new()
                    .translate(Vec3f::new(-3f32, 1.8f32, 1f32))
                    .scale(Vec3f::new(1.5f32, 1.5f32, 1.5f32))
                    .build(),
            );

            let obj = RenderObject::new(Box::new(geometry), material.clone()).unwrap();
            scene.add_with(obj, LAYER_NORMAL + 2);
        }

        let camera = Camera::new();
        camera.make_perspective(1f32, std::f32::consts::PI / 2f32, 0.01f32, 1000f32);

        camera.look_at(
            Vec3f::new(0f32, 4f32, 4f32),
            Vec3f::zeros(),
            Vec3f::new(0f32, 1f32, 0f32),
        );

        let camera = Arc::new(camera);
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
            if let core::event::Event::Update(_) = &ev {
                let ctx = context.container.get::<egui::Context>().unwrap();
                egui::Window::new("Test")
                    .resizable(true)
                    .show(&ctx, |ui| ctx.settings_ui(ui));
            }
        }
    }
}

fn main() {
    env_logger::init();
    let context = RContext::new();

    let mut app = App::new(context);
    app.register_plugin(WindowPluginFactory::new(
        "render queue",
        Size::new(900, 720),
    ));
    app.register_plugin(HardwareRenderPluginFactory);
    app.register_plugin(EguiPluginFactory {});
    app.add_event_processor(Box::new(MainLogic::default()));
    app.run();
}
