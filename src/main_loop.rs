use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use winit::dpi::LogicalSize;

use crate::{
    core::{backends::wgpu_backend::WGPUResource, context::RContextRef},
    event::EventProcessor,
    geometry::StaticGeometry,
    loader::ResourceManager,
    modules::{hardware_renderer::HardwareRenderer, ModuleRenderer, RenderParameter},
    render::{
        camera::RenderAttachment,
        material::{
            egui::{EguiMaterialFace, EguiMaterialFaceBuilder},
            MaterialBuilder,
        },
        scene::{Object, LAYER_UI},
        Camera, Material, Scene,
    },
    types::{Color, Size, Vec3f, Vec4f},
    ui::{self, UIMesh, UITextures, UI},
};

struct MainLoopInner {
    renderer: Option<HardwareRenderer>,
    scene: Scene,
    gpu: Arc<WGPUResource>,

    ui_camera: Arc<Camera>,
    ui: UI,

    main_depth_texture: Option<(wgpu::Texture, Arc<wgpu::TextureView>)>,

    ui_textures: Option<UITextures>,
    ui_mesh: UIMesh,

    ui_materials: Option<HashMap<egui::TextureId, Arc<Material>>>,
    size: Size,
}

struct MainLoopEventProcessor {
    inner: Rc<RefCell<MainLoopInner>>,
}

pub struct MainLoop {
    inner: Rc<RefCell<MainLoopInner>>,
}

impl MainLoopInner {
    pub fn new(gpu: Arc<WGPUResource>) -> Self {
        let mut scene = Scene::new(gpu.context_ref());
        let ui = UI::new();
        let ui_camera = Arc::new(Camera::new());
        ui_camera.make_orthographic(Vec4f::new(0f32, 0f32, 1f32, 1f32), 0.1f32, 10f32);
        ui_camera.look_at(
            Vec3f::new(0f32, 0f32, 1f32),
            Vec3f::zeros(),
            Vec3f::new(0f32, 1f32, 0f32),
        );
        scene.set_layer_camera(LAYER_UI, ui_camera.clone());

        let ui_mesh = UIMesh::new();
        let ui_textures = UITextures::default();

        Self {
            renderer: Some(HardwareRenderer::new()),
            gpu: gpu.clone(),
            scene,
            ui_camera,
            ui,
            main_depth_texture: None,
            size: Size::new(1u32, 1u32),
            ui_materials: Some(HashMap::new()),
            ui_mesh,
            ui_textures: Some(ui_textures),
        }
    }
}

impl MainLoop {
    pub fn new(gpu: Arc<WGPUResource>) -> Self {
        Self {
            inner: Rc::new(RefCell::new(MainLoopInner::new(gpu))),
        }
    }

    pub fn internal_processors(&self) -> Vec<Box<dyn EventProcessor>> {
        let mut vec = vec![];
        let inner = self.inner.borrow();

        vec.push(inner.ui.event_processor());
        vec.push(Box::new(MainLoopEventProcessor {
            inner: self.inner.clone(),
        }));
        vec
    }
}

impl MainLoopEventProcessor {
    fn update(&mut self, delta: f64) {
        let inner = self.inner.borrow_mut();
        inner.scene.update(delta)
    }

    fn build_ui_objects(&mut self) {
        let mut inner = self.inner.borrow_mut();
        inner.scene.clear_layer_objects(LAYER_UI);

        let mut ui_materials = inner.ui_materials.take().unwrap();

        let mut ui_textures = inner.ui_textures.take().unwrap();
        let meshes =
            inner
                .ui_mesh
                .generate_mesh(&inner.ui, inner.gpu.clone(), inner.size, &mut ui_textures);

        for (mesh, texture_id) in meshes {
            let material = ui_materials.entry(texture_id).or_insert_with(|| {
                let view = ui_textures.get_view(texture_id);
                MaterialBuilder::default()
                    .with_face(
                        EguiMaterialFaceBuilder::default()
                            .with_texture(view)
                            .build(),
                    )
                    .build(inner.gpu.context())
            });

            let object = Object::new(
                Box::new(StaticGeometry::new(Arc::new(mesh))),
                material.clone(),
            );

            inner.scene.add_ui(object);
        }

        inner.ui_textures = Some(ui_textures);
        inner.ui_materials = Some(ui_materials);
    }

    fn render(&mut self) {
        self.build_ui_objects();

        let mut inner = self.inner.borrow_mut();
        let mut renderer = inner.renderer.take().unwrap();

        let surface = match inner.gpu.surface().get_current_texture() {
            Ok(v) => v,
            Err(e) => {
                log::error!("{}", e);
                return;
            }
        };
        let view = surface
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let (_, depth_view) = inner.main_depth_texture.as_ref().unwrap();
        let clear_color = inner.ui.clear_color();

        let attachment = RenderAttachment::new_with_color_depth(
            Arc::new(view),
            depth_view.clone(),
            Some(clear_color),
        );

        // bind textures
        for (_, objects) in inner.scene.layers() {
            if let Some(c) = objects.camera() {
                c.bind_render_attachment(attachment.clone());
            }
        }

        let p = RenderParameter {
            gpu: inner.gpu.clone(),
            scene: &mut inner.scene,
        };

        renderer.render(p);

        inner.renderer = Some(renderer);

        surface.present();
    }
}

impl EventProcessor for MainLoopEventProcessor {
    fn on_event(
        &mut self,
        source: &dyn crate::event::EventSource,
        event: &crate::event::Event,
    ) -> crate::event::ProcessEventResult {
        match event {
            crate::event::Event::Update(delta) => {
                // update egui
                self.update(*delta);
            }
            crate::event::Event::Render => {
                self.render();
                return crate::event::ProcessEventResult::Consumed;
            }
            crate::event::Event::Resized(size) => {
                let mut inner = self.inner.borrow_mut();
                inner.size = Size::new(size.x, size.y);

                // create depth texture
                let texture = inner
                    .gpu
                    .new_depth_texture(Some("depth texture"), Size::new(size.x, size.y));
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

                inner.main_depth_texture = Some((texture, Arc::new(view)));
                let s = source.window().inner_size();
                let ps: LogicalSize<u32> = s.to_logical(source.window().scale_factor());

                inner.ui_camera.make_orthographic(
                    Vec4f::new(0f32, 0f32, ps.width as f32, ps.height as f32),
                    0.1f32,
                    10f32,
                );
                inner.ui_camera.look_at(
                    Vec3f::new(0f32, 0f32, 1f32),
                    Vec3f::zeros(),
                    Vec3f::new(0f32, 1f32, 0f32),
                );
            }
            crate::event::Event::CustomEvent(ev) => match ev {
                crate::event::CustomEvent::Loaded(scene) => {
                    let scene = source.resource_manager().take(*scene);
                    let mut inner = self.inner.borrow_mut();
                    inner.scene = scene;
                    let cam = inner.ui_camera.clone();
                    inner.scene.set_layer_camera(LAYER_UI, cam);
                }
                _ => (),
            },
            _ => (),
        }
        crate::event::ProcessEventResult::Received
    }
}
