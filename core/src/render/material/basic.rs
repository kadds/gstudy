use std::sync::{Arc, Mutex};

use indexmap::IndexSet;

use crate::{
    backends::wgpu_backend::{GpuInputMainBuffers, GpuInputMainBuffersWithProps, WGPUResource},
    graph::rdg::{
        pass::*,
        resource::{ClearValue, ResourceOps},
        RenderPassBuilder,
    },
    material::{basic::*, Material, MaterialId},
    render::{
        common::{FramedCache, StaticMeshMerger},
        resolve_pipeline, ColorTargetBuilder, DrawCommands, PipelinePassResource,
        RenderDescriptorObject,
    },
    types::*,
    util::{any_as_u8_slice, any_as_u8_slice_array},
};

use super::{MaterialRenderContext, MaterialRenderer, MaterialRendererFactory, SetupResource};

pub struct MaterialGpuResource {
    global_bind_group: Arc<wgpu::BindGroup>,

    bind_groups: FramedCache<MaterialId, Arc<wgpu::BindGroup>>,

    template: Arc<Vec<tshader::Pass>>,
    pipeline: PipelinePassResource,
    // sampler: Option<wgpu::Sampler>,
    static_commands: DrawCommands,
    dynamic_commands: DrawCommands,
}

struct MeshMerger {
    static_mesh_merger: StaticMeshMerger,
    dynamic_mesh_buffer: GpuInputMainBuffersWithProps,
}

struct BasicMaterialHardwareRendererInner {
    material_pipelines_cache: FramedCache<String, MaterialGpuResource>,

    merger: Arc<Mutex<MeshMerger>>,

    tech: Arc<tshader::ShaderTech>,
    render_rank: IndexSet<String>,
}

pub struct BasicMaterialHardwareRenderer {
    inner: BasicMaterialHardwareRendererInner,
}

impl RenderPassExecutor for BasicMaterialHardwareRenderer {
    fn execute<'a>(&'a mut self, mut context: RenderPassContext<'a>) {
        let inner = &mut self.inner;
        let merger = inner.merger.clone();
        let merger = merger.lock().unwrap();

        let mut rank = IndexSet::new();  
        std::mem::swap(&mut rank, &mut inner.render_rank);
        let mut life = vec![];
        let mut pass = context.new_pass();

        for material_key in &rank {
            let mgr = inner
                .material_pipelines_cache
                .get(material_key.as_str())
                .unwrap()
                .clone();
            life.push(mgr.clone());
        }

        for (_, mgr) in rank.into_iter().zip(life.iter()) {
            mgr.static_commands.draw(
                &mut pass,
                merger.static_mesh_merger.index(),
                merger.static_mesh_merger.vertex(),
                merger.static_mesh_merger.vertex_props(),
            );

            mgr.dynamic_commands.draw(
                &mut pass,
                merger.dynamic_mesh_buffer.index(),
                merger.dynamic_mesh_buffer.vertex(),
                merger.dynamic_mesh_buffer.vertex_props(),
            );
        }
    }
}

impl BasicMaterialHardwareRenderer {
    pub fn prepare_material_pipeline(
        &mut self,
        gpu: &WGPUResource,
        camera: &wgpu::Buffer,
        material: &Material,
    ) -> &mut MaterialGpuResource {
        let label = self.label();
        let inner = &mut self.inner;

        let mat = material.face_by::<BasicMaterialFace>();
        let key = mat.variants_name();
        inner.material_pipelines_cache.get_mut_or(key, |_| {
            let variants = material.face_by::<BasicMaterialFace>().variants();
            let template = inner.tech.register_variant(gpu.device(), variants).unwrap();

            let mut ins = RenderDescriptorObject::new();

            if let Some(blend) = material.blend() {
                ins = ins.add_target(
                    ColorTargetBuilder::new(gpu.surface_format())
                        .set_blender(*blend)
                        .build(),
                );
            } else {
                ins = ins.add_target(ColorTargetBuilder::new(gpu.surface_format()).build());
            }
            let depth_format = wgpu::TextureFormat::Depth32Float;

            ins = ins.set_primitive(|p: &mut _| *p = *material.primitive());
            ins = ins.set_depth(depth_format, |depth: &mut _| {
                depth.depth_compare = wgpu::CompareFunction::Less;
                depth.depth_write_enabled = !material.is_transparent();
            });

            let pipeline = resolve_pipeline(gpu, template.clone(), ins);

            let global_bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
                label,
                layout: &pipeline.pass[0].get_bind_group_layout(0),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: camera,
                        offset: 0,
                        size: None,
                    }),
                }],
            });
            let mut static_commands = DrawCommands::new(64, 1);
            let mut dynamic_commands = DrawCommands::new(64, 1);
            let global_bind_group = Arc::new(global_bind_group);
            static_commands.set_global_bind_group(global_bind_group.clone());
            dynamic_commands.set_global_bind_group(global_bind_group.clone());

            MaterialGpuResource {
                template,
                pipeline,
                bind_groups: FramedCache::new(),
                global_bind_group,
                dynamic_commands,
                static_commands,
            }
        })
    }
    pub fn label(&self) -> Option<&'static str> {
        Some("basic material")
    }
}

impl MaterialRenderer for BasicMaterialHardwareRenderer {
    fn render_material<'a>(
        &mut self,
        ctx: &mut MaterialRenderContext<'a>,
        objects: &[u64],
        material: &Material,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let gpu = ctx.gpu;
        let mat = material.face_by::<BasicMaterialFace>();
        let inner = &mut self.inner;
        let merger = inner.merger.clone();
        let mut merger = merger.lock().unwrap();

        if objects.is_empty() {
            return;
        }

        inner.render_rank.insert(mat.variants_name().into());

        // prepare dynamic buffer
        let (index_bytes, vertex_bytes, vertex_props_bytes) = ctx
            .scene
            .calculate_bytes(objects.iter(), |obj| !obj.geometry().is_static());
        merger
            .dynamic_mesh_buffer
            .prepare(gpu, index_bytes, vertex_bytes, vertex_props_bytes);

        let mgr = self.prepare_material_pipeline(gpu, ctx.main_camera, material);
        let pipeline = mgr.pipeline.pass[0].clone();
        let bind_group = mgr.bind_groups.get_or(material.id(), |key| {
            let label = if !material.name().is_empty() {
                material.name().into()
            } else {
                format!("basic material {:?}", key)
            };
            let label = Some(label.as_str());
            // material uniform buffer
            let material_uniform = if let Some(alpha) = material.alpha_test() {
                let buf = gpu.new_wvp_buffer::<ConstParameterWithAlpha>(label);
                let constp = ConstParameterWithAlpha {
                    color: mat.color(),
                    alpha,
                    _pad: Vec3f::zeros(),
                };
                gpu.queue().write_buffer(&buf, 0, any_as_u8_slice(&constp));
                buf
            } else {
                let buf = gpu.new_wvp_buffer::<ConstParameter>(label);
                let constp = ConstParameter { color: mat.color() };
                gpu.queue().write_buffer(&buf, 0, any_as_u8_slice(&constp));
                buf
            };

            let mut entries = vec![wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &material_uniform,
                    offset: 0,
                    size: None,
                }),
            }];

            let sampler = if mat.variants().iter().any(|v| v.need_sampler()) {
                let sampler = if let Some(sampler) = mat.sampler() {
                    sampler.sampler()
                } else {
                    gpu.default_sampler()
                };
                Some(sampler)
            } else {
                None
            };
            if let Some(sampler) = &sampler {
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        mat.texture()
                            .as_ref()
                            .map_or_else(|| gpu.default_texture(), |v| v.texture_view()),
                    ),
                })
            }

            let bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
                label,
                layout: &pipeline.get_bind_group_layout(1),
                entries: &entries,
            });
            Arc::new(bind_group)
        });

        let bind_groups = &[bind_group.clone()];

        // copy stage buffer

        let dynamic_pipeline = mgr.dynamic_commands.add_pipeline(pipeline.clone());
        let static_pipeline = mgr.static_commands.add_pipeline(pipeline);
        let container = ctx.scene.get_container();

        for id in objects {
            let object = container.get(id).unwrap();
            let object = object.o();

            let mesh = object.geometry().mesh();

            let mut command = if !object.geometry().is_static() {
                let (index, vertex, vertex_props) = merger.dynamic_mesh_buffer.copy_stage(
                    encoder,
                    gpu,
                    mesh.indices(),
                    mesh.vertices(),
                    mesh.vertices_props(),
                );
                let mut cmd = mgr.dynamic_commands.new_index_draw_command(
                    *id,
                    index,
                    vertex,
                    vertex_props,
                    mesh.index_count(),
                );
                cmd.set_pipeline(dynamic_pipeline);
                cmd
            } else {
                let (index, vertex, vertex_props) = merger.static_mesh_merger.write_cached(
                    gpu,
                    *id,
                    object.geometry().mesh_version(),
                    mesh.indices(),
                    mesh.vertices(),
                    mesh.vertices_props(),
                );
                let mut cmd = mgr.static_commands.new_index_draw_command(
                    *id,
                    index,
                    vertex,
                    vertex_props,
                    mesh.index_count(),
                );
                cmd.set_pipeline(static_pipeline);
                cmd
            };

            command.set_bind_groups(bind_groups);
            let constant = *object.geometry().transform().mat();
            command.set_constant(any_as_u8_slice(&constant));

            command.build();
        }
    }

    fn before_render(&mut self) {
        let inner = &mut self.inner;
        inner.material_pipelines_cache.recall();
        let merger = inner.merger.clone();
        let mut merger = merger.lock().unwrap();
        merger.dynamic_mesh_buffer.recall();
        merger.static_mesh_merger.recall();

        for (_, mgr) in inner.material_pipelines_cache.iter_mut() {
            let b = mgr.global_bind_group.clone();
            mgr.dynamic_commands.clear();
            mgr.static_commands.clear();
            mgr.dynamic_commands.set_global_bind_group(b.clone());
            mgr.static_commands.set_global_bind_group(b.clone());
        }
    }

    fn finish_render(&mut self) {
        let inner = &mut self.inner;
        let merger = inner.merger.clone();
        let mut merger = merger.lock().unwrap();
        merger.dynamic_mesh_buffer.finish();
    }
}

#[derive(Default)]
pub struct BasicMaterialRendererFactory {}

impl MaterialRendererFactory for BasicMaterialRendererFactory {
    fn setup(
        &self,
        _pass_ident: crate::render::PassIdent,
        material: &[&Material],
        gpu: &WGPUResource,
        g: &mut crate::graph::rdg::RenderGraphBuilder,
        setup_resource: &SetupResource,
    ) -> Arc<Mutex<dyn MaterialRenderer>> {
        let label = Some("basic");
        let tech = setup_resource
            .shader_loader
            .load_tech("basic_forward")
            .unwrap();

        let r = Arc::new(Mutex::new(BasicMaterialHardwareRenderer {
            inner: BasicMaterialHardwareRendererInner {
                tech,
                material_pipelines_cache: FramedCache::new(),
                merger: Arc::new(Mutex::new(MeshMerger {
                    static_mesh_merger: StaticMeshMerger::new(label),
                    dynamic_mesh_buffer: GpuInputMainBuffersWithProps::new(gpu, label),
                })),
                render_rank: IndexSet::new(),
            },
        }));

        let mut pass = RenderPassBuilder::new("basic render pass");
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

    // zorder 8bits
    // shader 8bits
    // ----------
    // pso_id 32bits
    fn sort_key(&self, material: &Material, gpu: &WGPUResource) -> u64 {
        0
    }
}
