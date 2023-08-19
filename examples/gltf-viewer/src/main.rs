use core::{
    context::RContext,
    geometry::{MeshBuilder, StaticGeometry},
    material::{basic::BasicMaterialFaceBuilder, MaterialBuilder},
    scene::{
        camera::{CameraController, TrackballCameraController},
        Camera, RenderObject, Scene,
    },
    types::{Size, Vec3f, Vec4f},
};
use std::{any::Any, cell::RefCell, sync::Arc};

use app::{container::Container, App, AppEventProcessor};
use egui_render::EguiPluginFactory;
use gltfloader::{GltfPluginFactory, Loader};
use window::{HardwareRenderPluginFactory, WindowPluginFactory};

#[derive(Default)]
pub struct MainLogic {
    reset_camera: Option<Arc<Camera>>,
    controller: Option<Box<RefCell<dyn CameraController>>>,
}

impl MainLogic {
    fn on_startup(&mut self, scene: &core::scene::Scene) {
        let camera = Camera::new(&scene.context());
        // camera.make_orthographic(Vec4f::new(1f32, -1f32, -1f32, 1f32), 0.1f32, 7f32);
        camera.make_perspective(1f32, std::f32::consts::PI / 2f32, 0.01f32, 100f32);

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
        } else if let Some(ev) = event.downcast_ref::<core::event::Event>() {
            match ev {
                core::event::Event::Update(_) => {
                    let ctx = context.container.get::<egui::Context>().unwrap();
                    self.draw_egui(&ctx, context.container);
                }
                core::event::Event::Input(input) => {
                    if let Some(c) = &mut self.controller {
                        c.borrow_mut().on_input(input);
                    }
                }
                _ => (),
            }
        } else if let Some(ev) = event.downcast_ref::<gltfloader::Event>() {
            if let gltfloader::Event::Loaded(res) = ev {
                if let Some(scene) = res.scene.clone() {
                    let s = context.container.get::<Scene>().unwrap();
                    let main_camera = s.main_camera_ref().unwrap();

                    self.reset_camera = Some(Arc::new((*main_camera).clone()));

                    self.controller = Some(Box::new(RefCell::new(TrackballCameraController::new(
                        main_camera.clone(),
                    ))));

                    s.remove_all();
                    s.extend(&scene);

                    s.set_main_camera(main_camera);
                }
            }
        }
    }
}

impl MainLogic {
    fn main_side(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, container: &Container) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Load scene").clicked() {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let file = rfd::FileDialog::new()
                            .add_filter("gltf", &["gltf", "glb"])
                            .set_title("load gltf file")
                            .pick_file();

                        if let Some(file) = file {
                            let loader = container.get::<Loader>().unwrap();
                            loader.load_async(file.to_str().unwrap_or_default());
                        }
                    }
                    ui.close_menu();
                }
                if ui.button("Clear scene").clicked() {
                    container.get::<Scene>().unwrap().remove_all();
                    ui.close_menu();
                }
            });
            ui.menu_button("Camera", |ui| {
                if ui.button("reset").clicked() {
                    if let Some(c) = &self.reset_camera {
                        let camera = Arc::new((**c).clone());
                        container
                            .get::<Scene>()
                            .unwrap()
                            .set_main_camera(camera.clone());
                        self.controller = Some(Box::new(RefCell::new(
                            TrackballCameraController::new(camera),
                        )));
                        ui.close_menu();
                    }
                }
            });
        });
    }

    fn draw_egui(&mut self, ctx: &egui::Context, container: &Container) {
        egui::Window::new("Control")
            .min_width(180f32)
            .default_width(240f32)
            .show(ctx, |ui| self.main_side(ctx, ui, container));
    }
}

fn main() {
    env_logger::init();
    let context = RContext::new();

    let mut app = App::new(context);
    app.register_plugin(WindowPluginFactory::new(
        "gltf viewer",
        Size::new(1300, 900),
    ));
    app.register_plugin(HardwareRenderPluginFactory);
    app.register_plugin(EguiPluginFactory);
    app.register_plugin(GltfPluginFactory);
    app.add_event_processor(Box::new(MainLogic::default()));
    app.run();
}
