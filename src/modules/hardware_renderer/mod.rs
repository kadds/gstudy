use std::{any::TypeId, collections::HashMap, sync::Arc};

use crate::{
    backends::wgpu_backend::{self, WGPURenderTarget},
    render::{
        material::{BasicMaterial, ConstantMaterial, DepthMaterial},
        Material,
    },
    types::Color,
};

use self::material::{
    BasicMaterialHardwareRenderer, DepthMaterialHardwareRenderer, MaterialRenderContext,
    MaterialRenderer,
};

use super::{ModuleFactory, ModuleInfo, ModuleRenderer, RenderParameter};

pub mod common;
mod material;

pub struct HardwareRenderer {
    material_renderer: HashMap<TypeId, Box<dyn MaterialRenderer>>,
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
        let mut material_renderer = HashMap::<TypeId, Box<dyn MaterialRenderer>>::new();
        material_renderer.insert(
            BasicMaterial::static_type_id(),
            Box::new(BasicMaterialHardwareRenderer::new()),
        );
        material_renderer.insert(
            DepthMaterial::static_type_id(),
            Box::new(DepthMaterialHardwareRenderer::new()),
        );
        material_renderer.insert(
            ConstantMaterial::static_type_id(),
            Box::new(BasicMaterialHardwareRenderer::new()),
        );

        Self {
            material_renderer,
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
        for (material_type, group) in mo {
            if let Some(r) = self.material_renderer.get_mut(material_type) {
                for (m, v) in &group.map {
                    let material = p.scene.get_material(*m).unwrap();
                    // let objects = group.get_objects_from_material(*m);

                    r.render_material(&mut ctx, v, material.as_ref());
                }
            }
        }
    }

    fn stop(&mut self) {}
}
