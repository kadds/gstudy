use crate::render::material::BasicMaterial;

use self::material::{BasicMaterialHardwareRenderer, MaterialRenderContext, MaterialRenderer};

use super::{ModuleFactory, ModuleInfo, ModuleRenderer, RenderParameter};

pub mod common;
mod material;

pub struct HardwareRenderer {
    basic_material: BasicMaterialHardwareRenderer,
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
        }
    }
}

impl ModuleRenderer for HardwareRenderer {
    fn render(&mut self, p: RenderParameter) {
        if !p.canvas.prepared() {
            return;
        }
        let label = Some("hardware renderer");
        let mut encoder = p
            .gpu_context
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label });
        let default_render_pass_desc = wgpu::RenderPassDescriptor {
            label,
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: p.canvas.get_texture().1,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        };
        let last_render_pass_desc = wgpu::RenderPassDescriptor {
            label,
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: p.canvas.get_texture().1,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        };
        let mut ctx = MaterialRenderContext {
            gpu_context: &p.gpu_context,
            camera: p.camera,
            scene: p.scene,
            encoder: &mut encoder,
            final_pass_desc: &default_render_pass_desc,
        };

        let mo = p.scene.load_material_objects();
        for (material_id, objects) in mo {
            if *material_id == BasicMaterial::self_type_id() {
                self.basic_material.render(&mut ctx, objects);
            }
            ctx.final_pass_desc = &last_render_pass_desc;
        }
        p.gpu_context
            .queue()
            .submit(std::iter::once(encoder.finish()))
    }

    fn stop(&mut self) {}
}
