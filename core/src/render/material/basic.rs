use std::{any::Any, collections::HashMap, num::NonZeroU64, sync::{Mutex, Arc}};

use nalgebra::Matrix4;
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use crate::{
    backends::wgpu_backend::{
        FsTarget, GpuInputMainBuffers, GpuInputMainBuffersWithUniform, GpuInputUniformBuffers,
        PipelineReflector, WGPUResource,
    },
    context::RContext,
    geometry::{Attribute, Mesh, MeshCoordType},
    material::{basic::*, Material, MaterialId},
    render::{
        common::{StaticMeshMerger, VertexDataGenerator}, RenderDescriptorObject, ColorTargetBuilder, resolve_pipeline, PipelinePassResource, DrawCommands,
    },
    scene::{Camera},
    types::*,
    util::{any_as_u8_slice, any_as_u8_slice_array}, graph::rdg::RenderPassBuilder,
};

use super::{BufferCache, MaterialRenderContext, MaterialRenderer, MaterialRendererFactory};

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


struct MaterialGpuResource {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    template: Arc<Vec<tshader::Pass>>,
    pipeline: PipelinePassResource,
    sampler: Option<wgpu::Sampler>,
}

struct BasicMaterialHardwareRendererInner {
    material_pipelines: HashMap<MaterialId, MaterialGpuResource>,

    // uniform_vp: wgpu::Buffer,

    // vp_bind_group: Option<wgpu::BindGroup>,

    static_mesh_merger: StaticMeshMerger,
    dynamic_mesh_buffer: GpuInputMainBuffers,
    uniform_buffer: GpuInputUniformBuffers,

    bind_group_for_objects: Vec<wgpu::BindGroup>,

    tech: Arc<tshader::ShaderTech>,
    commands: DrawCommands,
}

pub struct BasicMaterialHardwareRenderer {
    inner: BasicMaterialHardwareRendererInner,
}

impl BasicMaterialHardwareRenderer {
    pub fn prepare_material_pipeline(
        &mut self,
        gpu: &WGPUResource,
        material: &Material,
    ) {
        let label = self.label();
        let inner = &mut self.inner;

        let mat = material.face_by::<BasicMaterialFace>();
        let entry = inner.material_pipelines.entry(material.id());
        let pipe = entry.or_insert_with(|| {
            let mut variants =material.face_by::<BasicMaterialFace>().variants();
            let template = inner.tech.register_variant(gpu.device(), variants).unwrap();

            let mut obj = RenderDescriptorObject::new();

            if let Some(blend) = material.blend() {
                obj = obj.add_target(ColorTargetBuilder::new(gpu.surface_format()).set_default_blender().build());
            } else {
                obj = obj.add_target(ColorTargetBuilder::new(gpu.surface_format()).build());
            }
            let depth_format = wgpu::TextureFormat::Depth32Float;

            obj = obj.set_primitive(|p: &mut _| *p = *material.primitive());
            obj = obj.set_depth(depth_format, |depth: &mut _| {depth.depth_compare = wgpu::CompareFunction::Less;
                depth.depth_write_enabled = !material.is_transparent();
             });

            let pipeline = resolve_pipeline(gpu, template, obj);


            let buf = if let Some(alpha) = material.alpha_test() {
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
                    buffer: &buf,
                    offset: 0,
                    size: None,
                }),
            }];

            let sampler = if variants.iter().any(|v| v.need_sampler())
            {
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
                            .internal_view(),
                    ),
                })
            }

            let bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
                label,
                layout: &pipeline.pass[0].get_bind_group_layout(1),
                entries: &entries,
            });

            MaterialGpuResource {
                buffer: buf,
                sampler,
                template,
                pipeline,
                bind_group,
            }
        });

        let res = gpu.context().get_resource(pipe.pso.id());

        if inner.vp_bind_group.is_none() {
            let bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("basic material"),
                layout: &res.pso_ref().bind_group_layouts[0],
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &inner.uniform_vp,
                        offset: 0,
                        size: None,
                    }),
                }],
            });
            inner.vp_bind_group = Some(bind_group);
        }
    }
    pub fn label(&self) -> Option<&'static str> {
        Some("basic material")
    }
}

struct LazyVertexDataGenerator<'a> {
    data: Option<Vec<u8>>,
    mesh: &'a Mesh,
    face: &'a BasicMaterialFace,
}

impl<'a> VertexDataGenerator for LazyVertexDataGenerator<'a> {
    fn gen(&mut self) -> &[u8] {
        self.data = Some(zip_basic_input(self.face, self.mesh));
        self.data.as_ref().unwrap()
    }
}

impl MaterialRenderer for BasicMaterialHardwareRenderer {
    fn new_frame(&mut self, gpu: &WGPUResource) {}
    fn prepare_render(&mut self, gpu: &WGPUResource, camera: &Camera) {
        let inner = self.inner.get_or_insert_with(|| {
            let vp = gpu.new_wvp_buffer::<MVP>(Some("basic material"));
            BasicMaterialHardwareRendererInner {
                pipeline_pass: HashMap::new(),
                uniform_vp: vp,
                vp_bind_group: None,
                static_mesh_merger: StaticMeshMerger::new(Some(
                    "static basic material input buffer",
                )),
                dynamic_mesh_buffer: GpuInputMainBuffers::new(
                    gpu,
                    Some("basic material input buffer"),
                ),
                uniform_buffer: GpuInputUniformBuffers::new(
                    gpu,
                    Some("basic material uniform buffer"),
                ),
                bind_group_for_objects: Vec::new(),
            }
        });

        inner.uniform_buffer.finish();
        inner.uniform_buffer.recall();

        inner.dynamic_mesh_buffer.finish();
        inner.dynamic_mesh_buffer.recall();

        let wvp_data = WVP { mat: camera.vp() };

        gpu.queue()
            .write_buffer(&inner.uniform_vp, 0, any_as_u8_slice(&wvp_data));
    }

    fn render_material<'a>(
        &mut self,
        ctx: &mut MaterialRenderContext<'a>,
        objects: &[u64],
        material: &Material,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let face = material.face_by::<BasicMaterialFace>();
        let gpu = ctx.gpu;
        let label = self.label();

        self.prepare_material_pipeline(gpu, material);

        let inner = &mut self.inner;
        let mut mgr = inner.material_pipelines.get(&material.id()).unwrap();

        // prepare dynamic buffer
        let mut total_bytes = (0, 0);
        for id in objects {
            let object = ctx.scene.get_object(*id).unwrap();
            let mesh = object.geometry().mesh();
            let vertices = zip_basic_input_size(face, &mesh);
            let indices = mesh.indices();
            if !object.geometry().is_static() {
                total_bytes = (
                    total_bytes.0 + indices.len(),
                    total_bytes.1 + vertices as usize,
                );
            }
        }
        let uniform_changed = inner.uniform_buffer.make_sure(
            gpu,
            objects.len() as u64,
            std::mem::size_of::<Model>() as u64,
        );

        if uniform_changed {
            let buffers = inner.uniform_buffer.buffers();
            let mut new_bind_groups = Vec::new();

            for buffer in buffers {
                let bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
                    label,
                    layout: &pipe_res.pso_ref().bind_group_layouts[2],
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer,
                            offset: 0,
                            size: NonZeroU64::new(std::mem::size_of::<Model>() as u64),
                        }),
                    }],
                });
                new_bind_groups.push(bind_group);
            }
            std::mem::swap(&mut inner.bind_group_for_objects, &mut new_bind_groups);
        }

        // copy stage buffer
        let mut object_info = Vec::with_capacity(objects.len());

        for id in objects {
            let object = ctx.scene.get_object(*id).unwrap();
            let mesh = object.geometry().mesh();
            let model = Model {
                model: object.geometry().transform().mat().clone(),
            };
            let uniforms = any_as_u8_slice(&model);
            let result = if !object.geometry().is_static() {
                let vertices = &zip_basic_input(face, &mesh);
                let indices = mesh.indices();
                inner.dynamic_mesh_buffer.copy_stage(
                    ctx.encoder.encoder_mut(),
                    gpu,
                    indices,
                    vertices,
                )
            } else {
                let indices = mesh.indices();
                inner.static_mesh_merger.write_cached(
                    gpu,
                    *id,
                    object.geometry().mesh_version(),
                    indices,
                    LazyVertexDataGenerator {
                        data: None,
                        face,
                        mesh: &mesh,
                    },
                )
            };
            let uniform_range =
                inner
                    .uniform_buffer
                    .copy_stage(ctx.encoder.encoder_mut(), gpu, uniforms);

            object_info.push((result.0, result.1, uniform_range));
        }

        // draw
        let mut pass = ctx.encoder.new_pass();
        pass.set_pipeline(&pipe_res.pso_ref().pipeline);
        pass.set_bind_group(0, inner.vp_bind_group.as_ref().unwrap(), &[0]);
        pass.set_bind_group(1, &mgr.bind_group, &[0]);

        for (id, (index_range, vertex_range, uniform_position)) in objects.iter().zip(object_info) {
            let object = ctx.scene.get_object(*id).unwrap();
            let mesh = object.geometry().mesh();
            let index_count = mesh.index_count();
            pass.set_bind_group(
                2,
                &inner.bind_group_for_objects[uniform_position.index as usize],
                &[uniform_position.range.start as u32],
            );

            if !object.geometry().is_static() {
                pass.set_index_buffer(
                    inner.dynamic_mesh_buffer.index_buffer_slice(index_range),
                    wgpu::IndexFormat::Uint32,
                );
                pass.set_vertex_buffer(
                    0,
                    inner.dynamic_mesh_buffer.vertex_buffer_slice(vertex_range),
                );
            } else {
                pass.set_index_buffer(
                    inner
                        .static_mesh_merger
                        .index_buffer_slice(*id, index_range),
                    wgpu::IndexFormat::Uint32,
                );
                pass.set_vertex_buffer(
                    0,
                    inner
                        .static_mesh_merger
                        .vertex_buffer_slice(*id, vertex_range),
                );
            }

            pass.draw_indexed(0..index_count, 0, 0..1);
        }
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
        shader_loader: &tshader::Loader,
    ) -> Arc<Mutex<dyn MaterialRenderer>> {
        let label = Some("basic");
        let tech = shader_loader.load_tech("basic_forward").unwrap();

        let r = Arc::new(Mutex::new(
            BasicMaterialHardwareRenderer {
                inner: BasicMaterialHardwareRendererInner {
                    tech,
                    material_pipelines: HashMap::new(),
                }
            }
        ));

        let texture = g.import_texture();

        let pass = RenderPassBuilder::new("basic render pass");
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
