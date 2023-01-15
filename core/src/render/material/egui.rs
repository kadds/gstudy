use std::sync::{Arc, Mutex};

use wgpu::{
    util::{BufferInitDescriptor, DeviceExt, StagingBelt},
    BindGroupDescriptor,
};

use crate::{
    backends::wgpu_backend::{
        FsTarget, GpuInputMainBuffers, GpuInputUniformBuffers, GpuMainBuffer, PipelineReflector,
        WGPUResource,
    },
    graph::rdg::{
        backend::GraphBackend, pass::RenderPassExecutor, RenderGraphBuilder, RenderPassBuilder,
        ResourceRegistry,
    },
    material::{egui::EguiMaterialFace, Material, MaterialId},
    ps::{
        BlendState, CompareFunction, CullFace, DepthDescriptor, DepthStencilDescriptor,
        PrimitiveStateDescriptor,
    },
    render::{DrawCommand, DrawCommandBuilder, DrawCommands, PassIdent},
    scene::Camera,
    types::Vec2f,
    util::any_as_u8_slice,
};

use super::{
    HardwareMaterialShaderCache, MaterialRenderContext, MaterialRenderer, MaterialRendererFactory,
};

macro_rules! include_egui_shader {
    ($name: tt) => {
        (
            include_bytes!(concat!("../../compile_shaders/ui/", $name, ".vert")),
            include_bytes!(concat!("../../compile_shaders/ui/", $name, ".frag")),
        )
    };
}

struct ScreeSize {
    wh: Vec2f,
}

struct EguiMaterialHardwareRendererInner {
    main_buffers: GpuInputMainBuffers,
    sampler: wgpu::Sampler,
    commands: DrawCommands,
}

pub struct EguiMaterialHardwareRenderer {
    inner: EguiMaterialHardwareRendererInner,
}

impl EguiMaterialHardwareRenderer {
    pub fn name() -> &'static str {
        "egui"
    }
}

impl RenderPassExecutor for EguiMaterialHardwareRenderer {
    fn execute<'a>(
        &'a mut self,
        registry: &ResourceRegistry,
        backend: &GraphBackend,
        pass: &mut wgpu::RenderPass<'a>,
    ) {
        let inner = &mut self.inner;
        let mut last_pipeline = u32::MAX;
        let commands = inner.commands.commands();
        pass.set_bind_group(0, inner.commands.get_global_bind_group().unwrap(), &[0]);

        for command in commands {
            if let Some(clip) = inner.commands.get_clip(&command) {
                pass.set_scissor_rect(clip.x, clip.y, clip.z, clip.w);
            }
            if last_pipeline != command.pipeline {
                last_pipeline = command.pipeline;
                pass.set_pipeline(inner.commands.get_pipeline(&command));
            }

            let mut bind_group_iter = inner.commands.get_bind_groups(&command);
            let g = bind_group_iter.next().unwrap();

            pass.set_bind_group(1, g, &[]);

            pass.set_index_buffer(
                inner.main_buffers.index_buffer_slice(command.index),
                wgpu::IndexFormat::Uint32,
            );
            pass.set_vertex_buffer(0, inner.main_buffers.vertex_buffer_slice(command.vertex));
            pass.draw_indexed(0..command.draw_count, 0, 0..1);
        }
        // let buffer = gpu.new_wvp_buffer::<ScreeSize>(label);
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

        let mat = material.face_by::<EguiMaterialFace>();

        let mut total_bytes = (0, 0);

        for id in objects {
            let object = ctx.scene.get_object(*id).unwrap();
            let mesh = object.geometry().mesh();
            let vertices = mesh.mixed_mesh();
            let indices = mesh.indices();
            total_bytes = (
                total_bytes.0 + indices.len(),
                total_bytes.1 + vertices.len(),
            );
        }
        let pipeline = ctx.cache.get("ui/ui", 0);

        // global bind group
        let global_bg = ctx
            .gpu
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("egui"),
                layout: &pipeline.pass[0].get_bind_group_layout(0),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: ctx.global_uniform,
                        offset: 0,
                        size: None,
                    }),
                }],
            });

        // material bind group
        let bg = ctx
            .gpu
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("egui"),
                layout: &pipeline.pass[0].get_bind_group_layout(1),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(mat.texture().texture_view()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&inner.sampler),
                    },
                ],
            });

        let bg = Arc::new(bg);
        let global_bg = Arc::new(global_bg);

        inner
            .main_buffers
            .make_sure(ctx.gpu, total_bytes.0 as u64, total_bytes.1 as u64);
        let pipeline_offset = inner
            .commands
            .push_cached_pipeline(pipeline.pass[0].clone());

        // copy staging buffer
        for id in objects {
            let object = ctx.scene.get_object(*id).unwrap();
            let mesh = object.geometry().mesh();
            let vertices = mesh.mixed_mesh();
            let indices = mesh.indices();

            let (index, vertex) = inner
                .main_buffers
                .copy_stage(encoder, ctx.gpu, indices, vertices);
            let mut command_builder = inner.commands.builder();
            command_builder.set_draw(index, vertex, mesh.index_count());
            command_builder = command_builder.with_pipeline_offset(pipeline_offset);

            command_builder = command_builder.with_bind_groups(&[bg.clone()]);

            if let Some(clip) = mesh.clip() {
                command_builder = command_builder.with_clip(clip);
            }
            command_builder.build();
        }

        inner.commands.set_global_bind_group(global_bg);
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
        cache: &mut HardwareMaterialShaderCache,
    ) -> Arc<Mutex<dyn MaterialRenderer>> {
        let egui_texture = g.import_texture();
        let label = Some("egui");

        let r = Arc::new(Mutex::new(EguiMaterialHardwareRenderer {
            inner: EguiMaterialHardwareRendererInner {
                main_buffers: GpuInputMainBuffers::new(gpu, label),
                sampler: gpu.new_sampler(label),
                commands: DrawCommands::new(0, 1),
            },
        }));

        let mut pass = RenderPassBuilder::new("egui pass");
        pass.read_texture(egui_texture);
        pass.bind_default_render_target();
        pass.async_execute(r.clone());

        g.add_render_pass(pass.build());

        cache.resolve("ui/ui", 0, gpu, |b| {
            let fs_target = FsTarget::new_with_blend(
                gpu.surface_format(),
                &BlendState::default_append_blender(),
            );
            b.add_fs_target(fs_target)
                .with_depth(&DepthStencilDescriptor::default().with_depth(
                    DepthDescriptor::default().with_compare(CompareFunction::LessEqual),
                ))
                .with_primitive(PrimitiveStateDescriptor::default().with_cull_face(CullFace::None))
        });
        r
    }

    fn sort_key(&self, material: &Material, gpu: &WGPUResource) -> u64 {
        0
    }
}
