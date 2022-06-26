use crate::{
    backends::wgpu_backend::{self, WGPURenderTarget},
    render::material::BasicMaterial,
    types::Color,
};

use self::material::{BasicMaterialHardwareRenderer, MaterialRenderContext, MaterialRenderer};

use super::{ModuleFactory, ModuleInfo, ModuleRenderer, RenderParameter};

pub mod common;
mod material;

pub struct HardwareRenderer {
    basic_material: BasicMaterialHardwareRenderer,
    render_target: WGPURenderTarget,
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
        Self {
            basic_material: BasicMaterialHardwareRenderer::new(),
            render_target: WGPURenderTarget::new("hardware renderer"),
        }
    }
}

impl ModuleRenderer for HardwareRenderer {
    fn render(&mut self, p: RenderParameter) {
        if !p.canvas.prepared() {
            return;
        }
        let mut renderer = wgpu_backend::WGPURenderer::new(p.gpu.clone());
        self.render_target.set_render_target(
            p.canvas.get_texture().1,
            Some(Color::new(0f32, 0f32, 0f32, 1f32)),
        );

        let mut encoder = renderer.begin(&mut self.render_target).unwrap();

        let mut ctx = MaterialRenderContext {
            gpu: &p.gpu,
            camera: p.camera,
            scene: p.scene,
            encoder: &mut encoder,
        };

        let mo = p.scene.load_material_objects();
        for (material_id, objects) in mo {
            if *material_id == BasicMaterial::self_type_id() {
                self.basic_material.render(&mut ctx, objects);
            }
        }
    }

    fn stop(&mut self) {}
}
