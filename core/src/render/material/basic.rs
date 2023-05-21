use std::{
    num::NonZeroU64,
    sync::{Arc, Mutex},
};

use nalgebra::Matrix4;

use crate::{
    backends::wgpu_backend::{GpuInputMainBuffers, GpuInputMainBuffersWithProps, WGPUResource},
    geometry::Mesh,
    graph::rdg::{pass::RenderPassExecutor, RenderPassBuilder},
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

#[repr(C)]
struct MVP {
    mvp: Matrix4<f32>,
}

#[repr(C)]
struct Model {
    model: Mat4x4f,
}

// fn zip_basic_input(face: &BasicMaterialFace, mesh: &Mesh) -> Vec<u8> {
//     let mut ret = Vec::new();
//     match face.shader_ex() {
//         BasicMaterialShader::None => {
//             for a in mesh.vertices.iter() {
//                 let input = BasicInput { vertices: *a };
//                 ret.extend_from_slice(any_as_u8_slice(&input));
//             }
//         }
//         BasicMaterialShader::Color => {
//             for (a, b) in mesh
//                 .vertices
//                 .iter()
//                 .zip(mesh.coord_vec4f(MeshCoordType::Color).unwrap().iter())
//             {
//                 let input = BasicInputC {
//                     vertices: *a,
//                     colors: *b,
//                 };
//                 ret.extend_from_slice(any_as_u8_slice(&input));
//             }
//         }
//         BasicMaterialShader::Texture => {
//             for (a, b) in mesh
//                 .vertices
//                 .iter()
//                 .zip(mesh.coord_vec2f(MeshCoordType::TexCoord).unwrap().iter())
//             {
//                 let input = BasicInputT {
//                     vertices: *a,
//                     textcoord: *b,
//                 };
//                 ret.extend_from_slice(any_as_u8_slice(&input));
//             }
//         }
//         BasicMaterialShader::ColorTexture => {
//             for ((a, b), c) in mesh
//                 .vertices
//                 .iter()
//                 .zip(mesh.coord_vec4f(MeshCoordType::Color).unwrap().iter())
//                 .zip(mesh.coord_vec2f(MeshCoordType::TexCoord).unwrap().iter())
//             {
//                 let input = BasicInputCT {
//                     vertices: *a,
//                     colors: *b,
//                     texcoord: *c,
//                 };
//                 ret.extend_from_slice(any_as_u8_slice(&input));
//             }
//         }
//     }
//     ret
// }

pub struct MaterialGpuResource {
    buffer: wgpu::Buffer,
    global_bind_group: Arc<wgpu::BindGroup>,

    bind_group: Arc<wgpu::BindGroup>,
    template: Arc<Vec<tshader::Pass>>,
    pipeline: PipelinePassResource,
    sampler: Option<wgpu::Sampler>,
}

struct BasicMaterialHardwareRendererInner {
    material_pipelines_cache: FramedCache<MaterialId, MaterialGpuResource>,

    // uniform_vp: wgpu::Buffer,

    // vp_bind_group: Option<wgpu::BindGroup>,
    static_mesh_merger: StaticMeshMerger,
    dynamic_mesh_buffer: GpuInputMainBuffersWithProps,

    // bind_group_for_objects: Vec<wgpu::BindGroup>,
    tech: Arc<tshader::ShaderTech>,
    static_commands: DrawCommands,
    dynamic_commands: DrawCommands,
}

pub struct BasicMaterialHardwareRenderer {
    inner: BasicMaterialHardwareRendererInner,
}

impl RenderPassExecutor for BasicMaterialHardwareRenderer {
    fn execute<'a>(
        &'a mut self,
        registry: &crate::graph::rdg::ResourceRegistry,
        backend: &crate::graph::rdg::backend::GraphBackend,
        pass: &mut wgpu::RenderPass<'a>,
    ) {
        let inner = &mut self.inner;
        inner.dynamic_commands.draw(
            pass,
            inner.dynamic_mesh_buffer.index(),
            inner.dynamic_mesh_buffer.vertex(),
            inner.dynamic_mesh_buffer.vertex_props(),
        );
        inner.static_commands.draw(
            pass,
            inner.static_mesh_merger.index(),
            inner.static_mesh_merger.vertex(),
            inner.static_mesh_merger.vertex_props(),
        );
    }
}

impl BasicMaterialHardwareRenderer {
    pub fn prepare_material_pipeline(
        &mut self,
        gpu: &WGPUResource,
        camera: &wgpu::Buffer,
        material: &Material,
    ) -> &MaterialGpuResource {
        let label = self.label();
        let inner = &mut self.inner;

        let mat = material.face_by::<BasicMaterialFace>();
        inner.material_pipelines_cache.get_or(material.id(), |_| {
            let variants = material.face_by::<BasicMaterialFace>().variants();
            let template = inner.tech.register_variant(gpu.device(), variants).unwrap();

            let mut ins = RenderDescriptorObject::new();

            if let Some(blend) = material.blend() {
                ins = ins.add_target(
                    ColorTargetBuilder::new(gpu.surface_format())
                        .set_default_blender()
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

            let sampler = if variants.iter().any(|v| v.need_sampler()) {
                let sampler = gpu.new_sampler_linear(label);
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
                            .expect("texture view resource not exist")
                            .texture_view(),
                    ),
                })
            }

            let bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
                label,
                layout: &pipeline.pass[0].get_bind_group_layout(1),
                entries: &entries,
            });

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

            MaterialGpuResource {
                buffer: material_uniform,
                sampler,
                template,
                pipeline,
                bind_group: Arc::new(bind_group),
                global_bind_group: Arc::new(global_bind_group),
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

        let mgr = self.prepare_material_pipeline(gpu, ctx.main_camera, material);
        let pipeline = mgr.pipeline.pass[0].clone();
        let bind_groups = &[mgr.global_bind_group.clone(), mgr.bind_group.clone()];

        let inner = &mut self.inner;
        inner.dynamic_commands.clear();
        inner.static_commands.clear();
        inner.dynamic_mesh_buffer.recall();

        // prepare dynamic buffer
        let (index_bytes, vertex_bytes, vertex_props_bytes) = ctx
            .scene
            .calculate_bytes(objects.iter(), |obj| !obj.geometry().is_static());

        // copy stage buffer

        let dynamic_pipeline = inner.dynamic_commands.add_pipeline(pipeline.clone());
        let static_pipeline = inner.static_commands.add_pipeline(pipeline);

        inner
            .dynamic_mesh_buffer
            .prepare(gpu, index_bytes, vertex_bytes, vertex_props_bytes);

        for id in objects {
            let object = ctx.scene.get_object(*id).unwrap();
            let mesh = object.geometry().mesh();

            let mut command = if !object.geometry().is_static() {
                let (index, vertex, vertex_props) = inner.dynamic_mesh_buffer.copy_stage(
                    encoder,
                    gpu,
                    mesh.indices(),
                    mesh.vertices(),
                    mesh.vertices_props(),
                );
                let mut cmd = inner.dynamic_commands.new_index_draw_command(
                    *id,
                    index,
                    vertex,
                    vertex_props,
                    mesh.index_count(),
                );
                cmd.set_pipeline(dynamic_pipeline);
                cmd
            } else {
                let (index, vertex, vertex_props) = inner.static_mesh_merger.write_cached(
                    gpu,
                    *id,
                    object.geometry().mesh_version(),
                    mesh.indices(),
                    mesh.vertices(),
                    mesh.vertices_props(),
                );
                let mut cmd = inner.static_commands.new_index_draw_command(
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

            command.build();
        }

        inner.dynamic_mesh_buffer.finish();
    }
}

#[derive(Default)]
pub struct BasicMaterialRendererFactory {}

impl MaterialRendererFactory for BasicMaterialRendererFactory {
    fn setup(
        &self,
        pass_ident: crate::render::PassIdent,
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
                static_mesh_merger: StaticMeshMerger::new(label),
                dynamic_mesh_buffer: GpuInputMainBuffersWithProps::new(gpu, label),
                dynamic_commands: DrawCommands::new(0, 2),
                static_commands: DrawCommands::new(0, 2),
            },
        }));

        let texture = g.import_texture("basic render pass default");

        let mut pass = RenderPassBuilder::new("basic render pass");
        pass.bind_default_render_target();
        pass.read_texture(texture);
        pass.async_execute(r.clone());

        g.add_render_pass(pass.build());

        r
    }

    // zorder 8bits
    // shader 8bits
    // ----------
    // pso_id 32bits
    fn sort_key(&self, material: &Material, gpu: &WGPUResource) -> u64 {
        let shader_id = material.face().shader_id();

        (material.id().id() as u64) | (shader_id << 48)
    }
}
