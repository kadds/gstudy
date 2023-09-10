use core::{
    context::RContext,
    scene::{
        controller::{CameraController, CameraControllerFactory},
        Camera, Scene,
    },
    types::{Size, Vec3f, Vec4f},
    util::{angle2rad, rad2angle},
};
use std::{any::Any, cell::RefCell, sync::Arc};

use app::{container::Container, App, AppEventProcessor};
use egui_render::EguiPluginFactory;
use gltfloader::{GltfPluginFactory, Loader};
use rfd::{FileDialog, MessageDialog};
use window::{
    HardwareRenderPluginFactory, MainWindowHandle, StatisticsResource, WindowPluginFactory,
};

#[derive(Default)]
struct CameraSideState {
    perspective: bool,
    fov: f32,
    aspect: f32,

    rect: Vec4f,

    near: f32,
    far: f32,

    controller: String,
}

#[derive(Default)]
pub struct MainLogic {
    reset_camera: Option<Camera>,
    cur_camera: Option<Arc<Camera>>,
    controller: Option<Box<RefCell<dyn CameraController>>>,
    show_camera_side: bool,
    camera_state: CameraSideState,
}

impl MainLogic {
    fn on_startup(&mut self, scene: &core::scene::Scene) {
        let camera = Camera::new();
        camera.make_perspective(1f32, angle2rad(80f32), 0.01f32, 100f32);

        camera.look_at(
            Vec3f::new(0f32, 0f32, 5f32),
            Vec3f::zeros(),
            Vec3f::new(0f32, 1f32, 0f32),
        );

        scene.set_main_camera(Arc::new(camera));
        self.cur_camera = scene.main_camera_ref();
        self.fill_camera_state();
    }

    fn fill_camera_state(&mut self) {
        if let Some(camera) = &self.cur_camera {
            self.camera_state.perspective = camera.is_perspective();
            self.camera_state.aspect = camera.aspect();
            self.camera_state.far = camera.far();
            self.camera_state.near = camera.near();
            self.camera_state.fov = camera.fovy();
        }
        // self.camera_state.rect = ca
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

                    s.remove_all();
                    // copy objects
                    s.extend(&scene);

                    // copy camera
                    let c = self.cur_camera.take().unwrap();
                    c.copy_from(&scene.main_camera_ref().unwrap());

                    self.reset_camera = Some((*c).clone());
                    s.set_main_camera(c.clone());
                    self.cur_camera = Some(c);

                    self.fill_camera_state();
                } else {
                    let main_window = context.container.get::<MainWindowHandle>().unwrap();
                    MessageDialog::new()
                        .set_parent(&*main_window)
                        .set_title(&res.name)
                        .set_description(&res.error_string)
                        .show();
                }
            }
        }
    }
}

impl MainLogic {
    fn load_window(&self, container: &Container, loader_name: &str) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let main_window = container.get::<MainWindowHandle>().unwrap();
            let file = FileDialog::new()
                .set_parent(&*main_window)
                .add_filter("gltf", &["gltf", "glb"])
                .set_title("load gltf file")
                .pick_file();

            if let Some(file) = file {
                let loader = container.get::<Loader>().unwrap();
                loader.load_async(file.to_str().unwrap_or_default(), loader_name);
            }
        }
    }
    fn main_side(
        &mut self,
        _ctx: &egui::Context,
        ui: &mut egui::Ui,
        container: &Container,
        fps: f32,
    ) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Load scene(basic)").clicked() {
                    self.load_window(container, "basic");
                    ui.close_menu();
                }
                if ui.button("Load scene(phong)").clicked() {
                    self.load_window(container, "phong");
                    ui.close_menu();
                }
                if ui.button("Load scene(pbr)").clicked() {
                    self.load_window(container, "pbr");
                    ui.close_menu();
                }

                if ui.button("Clear scene").clicked() {
                    container.get::<Scene>().unwrap().remove_all();
                    ui.close_menu();
                }
            });
            ui.menu_button("Camera", |ui| {
                if ui.button("show").clicked() {
                    self.show_camera_side = true;
                    ui.close_menu();
                }
            });
        });

        ui.label(format!("fps {}", fps));
    }

    fn camera_view(ui: &mut egui::Ui, camera: &Camera) {
        ui.horizontal(|ui| {
            ui.label("from: ");
            ui.label(format!("{:?}", camera.from()));
        });
        ui.horizontal(|ui| {
            ui.label("to: ");
            ui.label(format!("{:?}", camera.to()));
        });
        ui.horizontal(|ui| {
            ui.label("up: ");
            ui.label(format!("{:?}", camera.up()));
        });
        ui.horizontal(|ui| {
            ui.label("distance: ");
            ui.label(format!(
                "{}",
                nalgebra::distance(&camera.from().into(), &camera.to().into())
            ));
        });
    }

    fn camera_project(ui: &mut egui::Ui, state: &mut CameraSideState, camera: &Camera) {
        if ui
            .horizontal(|ui| {
                ui.selectable_value(&mut state.perspective, true, "Perspective");
                ui.selectable_value(&mut state.perspective, false, "Orthographic")
            })
            .response
            .changed()
        {
            if state.perspective {
                camera.make_perspective(state.aspect, state.fov, state.near, state.far);
            } else {
                camera.make_orthographic(state.rect, state.near, state.far);
            }
        }

        if state.perspective {
            ui.horizontal_wrapped(|ui| {
                ui.label("fov");
                let mut fov = rad2angle(state.fov);
                if ui
                    .add(egui::Slider::new(&mut fov, 40f32..=120f32).suffix("°"))
                    .changed()
                {
                    state.fov = angle2rad(fov);
                    camera.set_fov(state.fov);
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("aspect");
                if ui
                    .add(egui::Slider::new(&mut state.aspect, 0.01f32..=100f32).logarithmic(true))
                    .changed()
                {
                    camera.set_aspect(state.aspect);
                }
            });
        }

        ui.horizontal_wrapped(|ui| {
            ui.label("near");
            if ui
                .add(
                    egui::Slider::new(&mut state.near, 0.00001f32..=(state.far - 0.1f32))
                        .logarithmic(true),
                )
                .changed()
            {
                camera.set_near(state.near);
            }
        });

        ui.horizontal_wrapped(|ui| {
            ui.label("far");
            if ui
                .add(
                    egui::Slider::new(&mut state.far, (state.near + 0.1f32)..=100000f32)
                        .logarithmic(true),
                )
                .changed()
            {
                camera.set_near(state.far);
            }
        });
    }

    fn camera_control(
        ui: &mut egui::Ui,
        state: &mut CameraSideState,
        camera: &Arc<Camera>,
        controller_factory: &CameraControllerFactory,
        controller: &mut Option<Box<RefCell<dyn CameraController>>>,
    ) {
        let c = state.controller.clone();
        if egui::ComboBox::from_label("Controller")
            .selected_text(c)
            .show_ui(ui, |ui| {
                ui.set_min_width(80f32);

                let mut change = ui
                    .selectable_value(&mut state.controller, "".to_owned(), "None")
                    .changed();

                for name in controller_factory.list() {
                    if ui
                        .selectable_value(&mut state.controller, name.clone(), name)
                        .changed()
                    {
                        change = true;
                    }
                }
                change
            })
            .inner
            .unwrap_or_default()
        {
            if state.controller.is_empty() {
                log::info!("clear camera controller");
                *controller = None;
            } else {
                *controller = controller_factory.create(&state.controller, camera.clone());
            }
        };
    }

    fn camera_side(
        ui: &mut egui::Ui,
        container: &Container,
        state: &mut CameraSideState,
        camera: Option<Arc<Camera>>,
        controller: &mut Option<Box<RefCell<dyn CameraController>>>,
    ) -> bool {
        if camera.is_none() {
            ui.label("no camera");
            return false;
        }
        let factory = container.get::<_>().unwrap();
        let camera = camera.unwrap();
        let res = ui.collapsing("Projection", |ui| Self::camera_project(ui, state, &camera));
        res.body_returned.unwrap_or_default();

        ui.separator();
        ui.collapsing("View", |ui| Self::camera_view(ui, &camera));

        ui.separator();
        egui::CollapsingHeader::new("Controller")
            .default_open(true)
            .show(ui, |ui| {
                Self::camera_control(ui, state, &camera, &factory, controller)
            });

        ui.separator();
        ui.vertical_centered(|ui| {
            if ui.button("reset").clicked() {
                true
            } else {
                false
            }
        })
        .inner
    }

    fn draw_egui(&mut self, ctx: &egui::Context, container: &Container) {
        let fs = container.get::<StatisticsResource>().unwrap();
        let fps = fs.lock().unwrap().fps();

        egui::Window::new("Control")
            .min_width(180f32)
            .default_width(240f32)
            .show(ctx, |ui| self.main_side(ctx, ui, container, fps));

        let reset = egui::Window::new("Camera")
            .open(&mut self.show_camera_side)
            .show(ctx, |ui| {
                Self::camera_side(
                    ui,
                    container,
                    &mut self.camera_state,
                    self.cur_camera.clone(),
                    &mut self.controller,
                )
            });

        if let Some(reset) = reset {
            if let Some(r) = reset.inner {
                // reset camera
                if r {
                    if let Some(reset_camera) = self.reset_camera.clone() {
                        self.cur_camera.as_ref().unwrap().copy_from(&reset_camera);
                    }
                }
            }
        }
    }
}

fn do_main() {
    env_logger::init();
    let context = RContext::new();

    let mut app = App::new(context);
    app.register_plugin(WindowPluginFactory::new(
        "gltf viewer",
        Size::new(1300, 900),
    ));
    app.register_plugin(HardwareRenderPluginFactory);
    app.register_plugin(EguiPluginFactory {});
    app.register_plugin(GltfPluginFactory);
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
