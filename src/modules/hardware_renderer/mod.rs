use std::{any::TypeId, cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use crate::{
    core::backends::wgpu_backend::{self, WGPURenderTarget, WGPURenderer},
    render::{
        material::{basic::BasicMaterialFace, egui::EguiMaterialFace},
        Camera, Material,
    },
    types::{Color, Mat4x4f},
};

use self::material::{
    basic::{BasicMaterialHardwareRenderer, BasicMaterialRendererFactory},
    egui::{EguiMaterialHardwareRenderer, EguiMaterialRendererFactory},
    MaterialRendererFactory,
};
use self::material::{MaterialRenderContext, MaterialRenderer};

use super::{ModuleFactory, ModuleInfo, ModuleRenderer, RenderParameter};

pub mod common;
mod material;

struct WVP {
    mat: Mat4x4f,
}

struct SingleMaterialRenderer {
    factory: Box<dyn MaterialRendererFactory>,

    renderers: HashMap<usize, Box<dyn MaterialRenderer>>,
}

impl SingleMaterialRenderer {
    pub fn new<T: MaterialRendererFactory + Default + 'static>() -> Self {
        Self {
            factory: Box::new(T::default()),
            renderers: HashMap::new(),
        }
    }

    pub fn make_sure(&mut self, ins: usize) {
        if !self.renderers.contains_key(&ins) {
            self.renderers.insert(ins, self.factory.new());
        }
    }

    pub fn get(&self, ins: usize) -> &dyn MaterialRenderer {
        self.renderers.get(&ins).unwrap().as_ref()
    }
    pub fn get_mut(&mut self, ins: usize) -> &mut dyn MaterialRenderer {
        self.renderers.get_mut(&ins).unwrap().as_mut()
    }
}

pub struct HardwareRenderer {
    material_renderer_factory: HashMap<TypeId, SingleMaterialRenderer>,
}

pub struct HardwareRendererFactory {}

impl HardwareRendererFactory {
    pub fn new() -> Self {
        Self {}
    }
}

impl ModuleFactory for HardwareRendererFactory {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            name: "hardware gpu renderer",
            desc: "gpu pipeline",
        }
    }

    fn make_renderer(&self) -> Box<dyn super::ModuleRenderer> {
        Box::new(HardwareRenderer::new())
    }
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

            let ptr = camera.as_ref() as *const Camera;
            let camera_id = ptr.addr();
            let mut reuse = true;

            camera_targets.entry(camera_id).or_insert_with(|| {
                reuse = false;
                WGPURenderTarget::new("target level")
            });

            layer_targets.insert(*layer, (reuse, camera_id));
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
            let (_, camera_id) = layer_targets.get(&layer).unwrap();
            s.make_sure(*camera_id);

            s.get_mut(*camera_id).sort_key(m, &gpu)
        });

        // mark new frame
        for (_, s) in &mut self.material_renderer_factory {
            for (_, r) in &mut s.renderers {
                r.new_frame(&gpu);
            }
        }

        // render objects
        let mut renderer = WGPURenderer::new(gpu.clone());
        let mut has_clear = false;

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

            let (reuse, cam) = layer_targets.get(layer).unwrap();
            let hardware_render_target = camera_targets.get_mut(cam).unwrap();

            // set render context
            let depth_target = render_attachment.depth_attachment().unwrap();
            if !has_clear {
                hardware_render_target
                    .set_render_target(&color_target, render_attachment.clear_color());
                hardware_render_target.set_depth_target(&depth_target, Some(f32::MAX));
                has_clear = true;
            } else {
                hardware_render_target.set_render_target(&color_target, None);
                hardware_render_target.set_depth_target(&depth_target, None);
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
                };
                r.prepare_render(&mut ctx);

                r.render_material(&mut ctx, &objects.map[&material.id()], &material);
            }
        }
    }

    fn stop(&mut self) {}
}
