use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};

use crate::{
    backends::wgpu_backend::{self, WGPURenderTarget, WGPURenderer, WGPUResource},
    material::{basic::BasicMaterialFace, egui::EguiMaterialFace},
    scene::{camera::RenderAttachment, Scene},
    types::Mat4x4f,
};

use self::material::{
    basic::{BasicMaterialHardwareRenderer, BasicMaterialRendererFactory},
    egui::{EguiMaterialHardwareRenderer, EguiMaterialRendererFactory},
    MaterialRendererFactory,
};
use self::material::{MaterialRenderContext, MaterialRenderer};

pub struct RenderParameter<'a> {
    pub gpu: Arc<WGPUResource>,
    pub scene: &'a mut Scene,
}

pub trait ModuleRenderer {
    fn render(&mut self, parameter: RenderParameter);
    fn stop(&mut self);
}

pub trait ModuleFactory: Sync + Send {
    fn info(&self) -> ModuleInfo;
    fn make_renderer(&self) -> Box<dyn ModuleRenderer>;
}

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: &'static str,
    pub desc: &'static str,
}

pub mod common;
mod material;

struct WVP {
    mat: Mat4x4f,
}

struct SingleMaterialRenderer {
    factory: Box<dyn MaterialRendererFactory>,

    renderers: HashMap<u64, Box<dyn MaterialRenderer>>, // camera -> renderere
}

impl SingleMaterialRenderer {
    pub fn new<T: MaterialRendererFactory + Default + 'static>() -> Self {
        Self {
            factory: Box::new(T::default()),
            renderers: HashMap::new(),
        }
    }

    pub fn make_sure(&mut self, cam: u64) {
        if !self.renderers.contains_key(&cam) {
            self.renderers.insert(cam, self.factory.new());
        }
    }

    pub fn get(&self, cam: u64) -> &dyn MaterialRenderer {
        self.renderers.get(&cam).unwrap().as_ref()
    }
    pub fn get_mut(&mut self, cam: u64) -> &mut dyn MaterialRenderer {
        self.renderers.get_mut(&cam).unwrap().as_mut()
    }
}

pub struct HardwareRenderer {
    material_renderer_factory: HashMap<TypeId, SingleMaterialRenderer>,
}

enum RenderMethod {
    Forward,
    Deferred,
    ForwardPlus,
    DeferredPlus,
}

impl HardwareRenderer {
    pub fn new() -> Self {
        let mut material_renderer_factory = HashMap::<TypeId, SingleMaterialRenderer>::new();
        material_renderer_factory.insert(
            TypeId::of::<BasicMaterialFace>(),
            SingleMaterialRenderer::new::<BasicMaterialRendererFactory>(),
        );
        material_renderer_factory.insert(
            TypeId::of::<EguiMaterialFace>(),
            SingleMaterialRenderer::new::<EguiMaterialRendererFactory>(),
        );

        Self {
            material_renderer_factory,
        }
    }

    fn prepare_frame(&mut self, p: RenderParameter) {}
}

impl ModuleRenderer for HardwareRenderer {
    fn render(&mut self, p: RenderParameter) {
        let gpu = p.gpu.clone();
        let scene = p.scene;

        let mut layer_targets = HashMap::new();
        let mut camera_targets = HashMap::new();

        for (layer, objects) in scene.layers() {
            let camera = match objects.camera_ref() {
                Some(v) => v,
                None => continue,
            };

            let camera_id = camera.id();

            camera_targets
                .entry(camera_id)
                .or_insert_with(|| WGPURenderTarget::new("target level"));

            layer_targets.insert(*layer, camera_id);
        }

        // clean cameras last frame
        for (_, s) in &mut self.material_renderer_factory {
            let mut tobe_clean = Vec::new();
            for (camera_id, _) in &s.renderers {
                if !camera_targets.contains_key(camera_id) {
                    tobe_clean.push(*camera_id);
                }
            }
            for cam in tobe_clean {
                s.renderers.remove(&cam);
            }
        }

        // sort objects
        scene.sort_all(|layer, m| {
            let s = self
                .material_renderer_factory
                .get_mut(&m.face_id())
                .unwrap();
            let camera_id = layer_targets.get(&layer).unwrap();
            s.make_sure(*camera_id);

            s.get_mut(*camera_id).sort_key(m, &gpu)
        });

        // mark new frame
        for (_, s) in &mut self.material_renderer_factory {
            for (_, r) in &mut s.renderers {
                r.new_frame(&gpu);
            }
        }

        let mut cam_used = HashSet::new();

        for (layer, objects) in scene.layers() {
            let camera = match objects.camera() {
                Some(v) => v,
                None => continue,
            };
            let cam = layer_targets.get(layer).unwrap();

            for (_, material) in &objects.sorted_objects {
                let mat_renderers = self
                    .material_renderer_factory
                    .get_mut(&material.face_id())
                    .unwrap();

                if cam_used.insert((material.face_id(), *cam)) {
                    let r = mat_renderers.get_mut(*cam);

                    r.prepare_render(&gpu, camera);
                }
            }
        }

        // render objects
        let mut renderer = WGPURenderer::new(gpu.clone());
        let mut clear_attachment_ids = HashSet::new();

        for (layer, objects) in scene.layers() {
            let camera = match objects.camera() {
                Some(v) => v,
                None => continue,
            };

            let render_attachment = camera.render_attachment();
            let color_target = match render_attachment.color_attachment() {
                Some(v) => v,
                None => {
                    return;
                }
            };

            let cam = layer_targets.get(layer).unwrap();
            let hardware_render_target = camera_targets.get_mut(cam).unwrap();

            // set render context
            let depth_target = render_attachment.depth_attachment().unwrap();
            if objects.sorted_objects.is_empty() {
                continue;
            }

            if !clear_attachment_ids.contains(&render_attachment.id()) {
                hardware_render_target.set_render_target(
                    color_target.internal_view(),
                    render_attachment.clear_color(),
                );
                hardware_render_target.set_depth_target(
                    depth_target.internal_view(),
                    render_attachment.clear_depth(),
                );
                clear_attachment_ids.insert(render_attachment.id());
            } else {
                hardware_render_target.set_render_target(color_target.internal_view(), None);
                hardware_render_target.set_depth_target(depth_target.internal_view(), None);
            }

            let mut encoder = renderer.begin(hardware_render_target).unwrap();

            for (_, material) in &objects.sorted_objects {
                let mat_renderers = self
                    .material_renderer_factory
                    .get_mut(&material.face_id())
                    .unwrap();
                let r = mat_renderers.get_mut(*cam);

                let mut ctx = MaterialRenderContext {
                    gpu: p.gpu.as_ref(),
                    camera,
                    scene: scene,
                    encoder: &mut encoder,
                    hint_fmt: render_attachment.format(),
                };

                r.render_material(&mut ctx, &objects.map[&material.id()], &material);
            }
        }
    }

    fn stop(&mut self) {}
}
