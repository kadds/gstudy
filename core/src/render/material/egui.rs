use std::sync::{Arc, Mutex};

use crate::{
    backends::wgpu_backend::{GpuInputMainBuffers, NullBufferAccessor, WGPUResource},
    graph::rdg::{
        backend::GraphBackend, pass::RenderPassExecutor, RenderGraphBuilder, RenderPassBuilder,
        ResourceRegistry,
    },
    material::{egui::EguiMaterialFace, Material, MaterialId},
    render::{
        common::FramedCache, resolve_pipeline, ColorTargetBuilder, DrawCommands, PassIdent,
        PipelinePassResource, RenderDescriptorObject,
    },
    types::Vec2f,
};

use super::{MaterialRenderer, MaterialRendererFactory, SetupResource};

struct ScreeSize {
    wh: Vec2f,
}

struct EguiMaterialHardwareRendererInner {
    main_buffers: GpuInputMainBuffers,
    sampler: wgpu::Sampler,
    commands: DrawCommands,
    pipeline: PipelinePassResource,
    global_bind_group: Arc<wgpu::BindGroup>,
    tech: Arc<tshader::ShaderTech>,
    material_bind_group_cache: FramedCache<MaterialId, Arc<wgpu::BindGroup>>,
}

pub struct EguiMaterialHardwareRenderer {
    inner: EguiMaterialHardwareRendererInner,
}

impl EguiMaterialHardwareRenderer {}

impl RenderPassExecutor for EguiMaterialHardwareRenderer {
    fn execute<'a>(
        &'a mut self,
        registry: &ResourceRegistry,
        backend: &GraphBackend,
        pass: &mut wgpu::RenderPass<'a>,
    ) {
        let inner = &mut self.inner;
        inner.commands.draw(
            pass,
            inner.main_buffers.index(),
            inner.main_buffers.vertex(),
            NullBufferAccessor,
        );
    }
}

impl MaterialRenderer for EguiMaterialHardwareRenderer {
    fn render_material<'a, 'b>(
        &'a mut self,
        ctx: &'b mut super::MaterialRenderContext<'b>,
        objects: &'b [u64],
        material: &'b Material,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let inner = &mut self.inner;
        inner.main_buffers.recall();
        inner.material_bind_group_cache.recall();
        inner.commands.clear();

        let mat = material.face_by::<EguiMaterialFace>();

        let (index_bytes, vertex_bytes, vertex_props_bytes) =
            ctx.scene.calculate_bytes(objects.iter(), |_| true);

        // material bind group
        let bg = inner
            .material_bind_group_cache
            .get_or(material.id(), |_| {
                let bg = ctx
                    .gpu
                    .device()
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("egui"),
                        layout: &inner.pipeline.pass[0].get_bind_group_layout(1),
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(
                                    mat.texture().texture_view(),
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&inner.sampler),
                            },
                        ],
                    });
                Arc::new(bg)
            })
            .clone();

        inner
            .main_buffers
            .prepare(ctx.gpu, index_bytes, vertex_props_bytes);

        inner
            .commands
            .set_global_bind_group(inner.global_bind_group.clone());
        let default_pipeline = inner.commands.add_pipeline(inner.pipeline.pass[0].clone());

        // copy staging buffer
        for id in objects {
            let object = ctx.scene.get_object(*id).unwrap();
            let mesh = object.geometry().mesh();
            let vertices = mesh.vertices_props();
            let indices = mesh.indices();

            let (index_range, vertex_range) = inner
                .main_buffers
                .copy_stage(encoder, ctx.gpu, indices, vertices);

            let mut command_builder = inner.commands.new_index_draw_command(
                *id,
                index_range,
                vertex_range,
                0..0,
                mesh.index_count(),
            );
            command_builder.set_pipeline(default_pipeline);
            command_builder.set_bind_groups(&[bg.clone()]);

            if let Some(clip) = mesh.clip() {
                command_builder.set_clip(clip);
            }
            command_builder.build();
        }
        inner.main_buffers.finish();
    }
}

#[derive(Default)]
pub struct EguiMaterialRendererFactory {}

impl MaterialRendererFactory for EguiMaterialRendererFactory {
    fn setup(
        &self,
        ident: PassIdent,
        material: &[&Material],
        gpu: &WGPUResource,
        g: &mut RenderGraphBuilder,
        setup_resource: &SetupResource,
    ) -> Arc<Mutex<dyn MaterialRenderer>> {
        let egui_texture = g.import_texture("egui default");
        let label = Some("egui");
        let tech = setup_resource.shader_loader.load_tech("egui").unwrap();
        let template = tech.register_variant(gpu.device(), &[]).unwrap();
        let depth_format = wgpu::TextureFormat::Depth32Float;

        let pipeline = resolve_pipeline(
            gpu,
            template,
            RenderDescriptorObject::new()
                .set_depth(depth_format, |depth: &mut _| {
                    depth.depth_compare = wgpu::CompareFunction::LessEqual;
                })
                .set_primitive(|primitive: &mut _| {
                    primitive.cull_mode = None;
                })
                .add_target(
                    ColorTargetBuilder::new(gpu.surface_format())
                        .set_append_blender()
                        .build(),
                ),
        );

        // global bind group
        let global_bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("egui"),
            layout: &pipeline.pass[0].get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: setup_resource.ui_camera,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let r = Arc::new(Mutex::new(EguiMaterialHardwareRenderer {
            inner: EguiMaterialHardwareRendererInner {
                main_buffers: GpuInputMainBuffers::new(gpu, label),
                sampler: gpu.new_sampler(label),
                commands: DrawCommands::new(0, 1),
                tech,
                pipeline,
                global_bind_group: Arc::new(global_bind_group),
                material_bind_group_cache: FramedCache::new(),
            },
        }));

        let mut pass = RenderPassBuilder::new("egui pass");
        pass.read_texture(egui_texture);
        pass.bind_default_render_target();
        pass.async_execute(r.clone());

        g.add_render_pass(pass.build());

        r
    }

    fn sort_key(&self, material: &Material, gpu: &WGPUResource) -> u64 {
        0
    }
}
