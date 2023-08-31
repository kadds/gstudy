use core::{
    backends::wgpu_backend::{ClearValue, ResourceOps},
    graph::rdg::{
        pass::{
            ColorRenderTargetDescriptor, DepthRenderTargetDescriptor, PreferAttachment,
            RenderPassExecutor, RenderTargetDescriptor,
        },
        RenderPassBuilder,
    },
    material::MaterialId,
    render::{common::FramedCache, material::MaterialRendererFactory, PipelinePassResource},
};
use std::sync::{Arc, Mutex};

use tshader::{LoadTechConfig, ShaderTech};

use crate::light::SceneLights;

struct MaterialGpuResource {
    global_bind_group: wgpu::BindGroup,

    material_bind_buffers: FramedCache<MaterialId, wgpu::Buffer>,
    bind_groups: FramedCache<MaterialId, Option<wgpu::BindGroup>>,

    template: Arc<Vec<tshader::Pass>>,
    pipeline: PipelinePassResource,
}

struct PhongMaterialSharedData {
    tech: Arc<ShaderTech>,
    mass: u32,
    material_pipelines_cache: FramedCache<u64, MaterialGpuResource>,
}

struct PhongMaterialBaseRenderer {
    shared: Arc<Mutex<PhongMaterialSharedData>>,
}

impl RenderPassExecutor for PhongMaterialBaseRenderer {
    fn prepare<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        engine: &mut core::graph::rdg::backend::GraphCopyEngine,
    ) -> Option<()> {
        let shared = self.shared.lock().unwrap();

        None
    }

    fn queue<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        device: &wgpu::Device,
    ) {
    }

    fn render<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        engine: &mut core::graph::rdg::backend::GraphRenderEngine,
    ) {
    }

    fn cleanup<'b>(&'b mut self, context: core::graph::rdg::pass::RenderPassContext<'b>) {}
}

struct PhongMaterialAddRenderer {
    shared: Arc<Mutex<PhongMaterialSharedData>>,
}

impl RenderPassExecutor for PhongMaterialAddRenderer {
    fn prepare<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        engine: &mut core::graph::rdg::backend::GraphCopyEngine,
    ) -> Option<()> {
        None
    }

    fn queue<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        device: &wgpu::Device,
    ) {
    }

    fn render<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        engine: &mut core::graph::rdg::backend::GraphRenderEngine,
    ) {
    }

    fn cleanup<'b>(&'b mut self, context: core::graph::rdg::pass::RenderPassContext<'b>) {}
}

pub struct PhongMaterialRendererFactory {}

impl MaterialRendererFactory for PhongMaterialRendererFactory {
    fn setup(
        &self,
        materials: &[std::sync::Arc<core::material::Material>],
        gpu: &core::backends::wgpu_backend::WGPUResource,
        g: &mut core::graph::rdg::RenderGraphBuilder,
        setup_resource: &core::render::material::SetupResource,
    ) {
        let tech = setup_resource
            .shader_loader
            .load_tech(LoadTechConfig {
                name: "phong".into(),
            })
            .unwrap();

        let shared = PhongMaterialSharedData {
            tech,
            mass: setup_resource.msaa,
            material_pipelines_cache: FramedCache::new(),
        };

        let lights = setup_resource.scene.get_resource::<SceneLights>().unwrap();
        let mut light_variants = vec![];

        let extra_lights = lights.extra_lights();
        let has_direct_light = lights.has_direct_light();

        if has_direct_light {
            light_variants.push("DIRECT_LIGHT");
        }
        let mut base_pass = RenderPassBuilder::new("phong forward base pass");
        base_pass.default_color_depth_render_target();
        let shared = Arc::new(Mutex::new(shared));

        base_pass.async_execute(Arc::new(Mutex::new(PhongMaterialBaseRenderer {
            shared: shared.clone(),
        })));

        g.add_render_pass(base_pass);

        if extra_lights > 0 {
            // add pass
            for i in 1..=extra_lights {
                let mut add_pass = RenderPassBuilder::new(format!("phong forward add pass {}", i));
                add_pass.default_color_depth_render_target();

                add_pass.async_execute(Arc::new(Mutex::new(PhongMaterialAddRenderer {
                    shared: shared.clone(),
                })));

                g.add_render_pass(add_pass);
            }
        }
    }
}
