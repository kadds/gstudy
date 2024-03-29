use std::{
    mem::size_of,
    ops::Range,
    sync::{Arc, Mutex},
};

use core::{
    backends::wgpu_backend::{ClearValue, GpuInputMainBuffers, ResourceOps, WGPUResource},
    graph::rdg::{backend::GraphCopyEngine, pass::*, RenderGraphBuilder, RenderPassBuilder},
    material::{Material, MaterialId},
    render::{
        common::FramedCache,
        material::{take_rs, MaterialRendererFactory, RenderMaterialBuilderMap, SetupResource},
        resolve_pipeline, ColorTargetBuilder, PipelinePassResource, RenderDescriptorObject,
        ResolvePipelineConfig,
    },
    scene::LayerId,
    types::Rectu,
    wgpu,
};

use tshader::LoadTechConfig;

use crate::material::EguiMaterialFace;

struct EguiMaterialHardwareRendererInner {
    main_buffers: GpuInputMainBuffers,

    sampler: wgpu::Sampler,
    pipeline: PipelinePassResource,
    tech: Arc<tshader::ShaderTech>,
    material_bind_group_cache: FramedCache<MaterialId, wgpu::BindGroup>,

    draw_index_buffer: Vec<(Range<u32>, i32, Option<Rectu>)>,
}

pub struct EguiMaterialHardwareRenderer {
    inner: EguiMaterialHardwareRendererInner,
    layer: LayerId,
}

impl EguiMaterialHardwareRenderer {}

impl RenderPassExecutor for EguiMaterialHardwareRenderer {
    #[profiling::function]
    fn prepare<'a>(
        &'a mut self,
        context: RenderPassContext<'a>,
        engine: &mut GraphCopyEngine,
    ) -> Option<()> {
        let inner = &mut self.inner;
        inner.main_buffers.recall();
        let rs = take_rs::<EguiMaterialFace>(&context)?;
        let c = rs.scene.get_container();

        // copy vertices and indices
        let gpu_ref = engine.gpu_ref();

        let layer = rs.layer(self.layer);

        for indirect in &layer.material {
            let objects = layer.objects(indirect);

            for id in objects {
                let obj = match c.get(id) {
                    Some(v) => v,
                    None => continue,
                };
                let obj = obj.o();
                let mesh = obj.geometry().mesh();
                let indices = mesh.indices_view().unwrap();
                let vertices = mesh.properties_view();

                inner
                    .main_buffers
                    .prepare(&gpu_ref, indices.len() as u64, vertices.len() as u64);

                let (is, vs) =
                    inner
                        .main_buffers
                        .copy_stage(engine.encoder(), &gpu_ref, indices, vertices);

                let index_size = size_of::<u32>() as u64;
                let vs_size = mesh.row_strip_size() as u64;
                let vs = (vs.start / vs_size) as i32;

                inner.draw_index_buffer.push((
                    (is.start / index_size) as u32..(is.end / index_size) as u32,
                    vs,
                    mesh.clip(),
                ));
            }
        }

        self.inner.main_buffers.finish();

        Some(())
    }

    #[profiling::function]
    fn queue<'b>(&'b mut self, context: RenderPassContext<'b>, device: &wgpu::Device) {
        let inner = &mut self.inner;

        let rs = take_rs::<EguiMaterialFace>(&context).unwrap();
        let layer = rs.layer(self.layer);
        for indirect in &layer.material {
            let mat = indirect.material.face_by::<EguiMaterialFace>();

            inner
                .material_bind_group_cache
                .get_or(indirect.mat_id, |_| {
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
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
                    })
                });
        }
    }

    #[profiling::function]
    fn render<'b>(
        &'b mut self,
        context: RenderPassContext<'b>,
        engine: &mut core::graph::rdg::backend::GraphRenderEngine,
    ) {
        let inner = &mut self.inner;

        let rs = take_rs::<EguiMaterialFace>(&context).unwrap();
        let layer = rs.layer(self.layer);
        let mut pass = engine.begin(layer.layer);

        pass.set_pipeline(inner.pipeline.pass[0].render());
        pass.set_bind_group(0, &layer.main_camera.bind_group, &[]);
        pass.set_index_buffer(
            inner.main_buffers.index().buffer().slice(..),
            wgpu::IndexFormat::Uint32,
        );
        pass.set_vertex_buffer(0, inner.main_buffers.vertex().buffer().slice(..));
        for indirect in &layer.material {
            let material = indirect.material.as_ref();

            let material_bind_group = inner.material_bind_group_cache.get(&material.id()).unwrap();

            pass.set_bind_group(1, material_bind_group, &[]);

            for (indices, vertices, rect) in &inner.draw_index_buffer {
                if let Some(r) = rect {
                    pass.set_scissor_rect(r.x, r.y, r.z, r.w);
                }
                pass.draw_indexed(indices.clone(), *vertices, 0..1);
            }
        }
    }

    #[profiling::function]
    fn cleanup<'b>(&'b mut self, _context: RenderPassContext<'b>) {
        self.inner.draw_index_buffer.clear();
    }
}

#[derive(Default)]
pub struct EguiMaterialRendererFactory {}

impl MaterialRendererFactory for EguiMaterialRendererFactory {
    fn setup(
        &self,
        materials_map: &RenderMaterialBuilderMap,
        gpu: &WGPUResource,
        g: &mut RenderGraphBuilder,
        setup_resource: &SetupResource,
    ) {
        let label = Some("egui");
        let tech = setup_resource
            .shader_loader
            .load_tech(LoadTechConfig {
                name: "egui".into(),
            })
            .unwrap();
        let template = tech.register_variant(gpu.device(), &[&[]]).unwrap();
        let depth_format = wgpu::TextureFormat::Depth32Float;

        let pipeline = resolve_pipeline(
            gpu,
            &template,
            RenderDescriptorObject::new()
                .set_depth(depth_format, |depth: &mut _| {
                    depth.depth_compare = wgpu::CompareFunction::LessEqual;
                })
                .set_primitive(|primitive: &mut _| {
                    primitive.cull_mode = None;
                })
                .set_msaa(setup_resource.msaa)
                .add_target(
                    ColorTargetBuilder::new(gpu.surface_format())
                        .set_append_blender()
                        .build(),
                ),
            &ResolvePipelineConfig {
                global_bind_group_layout: Some(&setup_resource.ui_camera.bind_group_layout),
                ..Default::default()
            },
        );

        for (layer, _) in materials_map {
            let r = Arc::new(Mutex::new(EguiMaterialHardwareRenderer {
                inner: EguiMaterialHardwareRendererInner {
                    main_buffers: GpuInputMainBuffers::new(gpu, label),
                    sampler: gpu.new_sampler(label),
                    tech: tech.clone(),
                    pipeline: pipeline.clone(),
                    material_bind_group_cache: FramedCache::new(),
                    draw_index_buffer: vec![],
                },
                layer: *layer,
            }));

            let mut pass = RenderPassBuilder::new(format!("egui pass layer {}", layer));
            pass.render_target(RenderTargetDescriptor {
                colors: smallvec::smallvec![ColorRenderTargetDescriptor {
                    prefer_attachment: PreferAttachment::Default,
                    resolve_attachment: PreferAttachment::Default,
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

            pass.add_constraint(PassConstraint::Last);

            pass.async_execute(r.clone());

            g.add_render_pass(pass);
        }
    }
}
