use std::sync::{Arc, Mutex};

use core::{
    backends::wgpu_backend::{GpuInputMainBuffers, NullBufferAccessor, WGPUResource},
    graph::rdg::{
        pass::*,
        resource::{ClearValue, ResourceOps},
        RenderGraphBuilder, RenderPassBuilder,
    },
    material::{Material, MaterialId},
    render::{
        common::FramedCache,
        material::{
            MaterialRenderContext, MaterialRenderer, MaterialRendererFactory, SetupResource,
        },
        resolve_pipeline, ColorTargetBuilder, DrawCommands, PassIdent, PipelinePassResource,
        RenderDescriptorObject,
    },
};

use crate::material::EguiMaterialFace;

struct EguiMaterialHardwareRendererInner {
    main_buffers: GpuInputMainBuffers,
    sampler: wgpu::Sampler,
    pub(crate) commands: DrawCommands,
    pub(crate) pipeline: PipelinePassResource,
    global_bind_group: Arc<wgpu::BindGroup>,
    tech: Arc<tshader::ShaderTech>,
    material_bind_group_cache: FramedCache<MaterialId, Arc<wgpu::BindGroup>>,
}

pub struct EguiMaterialHardwareRenderer {
    inner: EguiMaterialHardwareRendererInner,
}

impl EguiMaterialHardwareRenderer {}

impl RenderPassExecutor for EguiMaterialHardwareRenderer {
    fn execute<'a>(&'a mut self, mut context: RenderPassContext<'a>) {
        let inner = &mut self.inner;
        let mut pass = context.new_pass();

        inner.commands.draw(
            &mut pass,
            inner.main_buffers.index(),
            inner.main_buffers.vertex(),
            NullBufferAccessor,
        );
    }
}

impl MaterialRenderer for EguiMaterialHardwareRenderer {
    fn render_material<'b>(
        &mut self,
        ctx: &'b mut MaterialRenderContext<'b>,
        objects: &'b [u64],
        material: &'b Material,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let inner = &mut self.inner;
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

        let default_pipeline = inner.commands.add_pipeline(inner.pipeline.pass[0].clone());
        let container = ctx.scene.get_container();

        // copy staging buffer
        for id in objects {
            let object = container.get(id).unwrap();
            let object = object.o();
            let mesh = object.geometry().mesh();
            let vertices = mesh.properties_view();
            let indices = mesh.indices_view().unwrap();

            let (index_range, vertex_range) = inner
                .main_buffers
                .copy_stage(encoder, ctx.gpu, indices, vertices);

            let mut command_builder = inner.commands.new_index_draw_command(
                *id,
                index_range,
                vertex_range,
                0..0,
                mesh.index_count().unwrap(),
            );
            command_builder.set_pipeline(default_pipeline);
            command_builder.set_bind_groups(&[bg.clone()]);

            if let Some(clip) = mesh.clip() {
                command_builder.set_clip(clip);
            }
            command_builder.build();
        }
    }

    fn before_render(&mut self) {
        let inner = &mut self.inner;
        inner.main_buffers.recall();
        inner.material_bind_group_cache.recall();
        inner.commands.clear();

        inner
            .commands
            .set_global_bind_group(inner.global_bind_group.clone());
    }

    fn finish_render(&mut self) {
        let inner = &mut self.inner;
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
        pass.render_target(RenderTargetDescriptor {
            colors: smallvec::smallvec![ColorRenderTargetDescriptor {
                prefer_attachment: PreferAttachment::Default,
                ops: ResourceOps {
                    load: None,
                    store: true,
                },
            }],
            depth: Some(DepthRenderTargetDescriptor {
                prefer_attachment: PreferAttachment::Default,
                depth_ops: Some(ResourceOps {
                    load: Some(ClearValue::Depth(1.0f32)),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        pass.async_execute(r.clone());

        g.add_render_pass(pass.build());

        r
    }

    fn sort_key(&self, _material: &Material, _gpu: &WGPUResource) -> u64 {
        0
    }
}
