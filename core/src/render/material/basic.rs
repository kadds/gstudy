use std::{any::Any, collections::HashMap, num::NonZeroU64};

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
    ps::PipelineStateObject,
    render::{
        common::{StaticMeshMerger, VertexDataGenerator},
        WVP,
    },
    scene::{Camera, Object},
    types::*,
    util::{any_as_u8_slice, any_as_u8_slice_array},
};

use super::{BufferCache, MaterialRenderContext, MaterialRenderer, MaterialRendererFactory};

#[repr(C)]
struct MVP {
    mvp: Matrix4<f32>,
}

#[repr(C)]
struct BasicInput {
    vertices: Vec3f,
}

#[repr(C)]
struct BasicInputC {
    vertices: Vec3f,
    colors: Vec4f,
}

#[repr(C)]
struct BasicInputT {
    vertices: Vec3f,
    textcoord: Vec2f,
}

#[repr(C)]
struct BasicInputCTN {
    vertices: Vec3f,
    colors: Vec4f,
    texcoord: Vec2f,
    normal: Vec2f,
}

#[repr(C)]
struct BasicInputCT {
    vertices: Vec3f,
    colors: Vec4f,
    texcoord: Vec2f,
}

#[repr(C)]
struct ConstParameter {
    color: Vec4f,
}

#[repr(C)]
struct ConstParameterWithAlpha {
    color: Vec4f,
    alpha: f32,
    _pad: Vec3f,
}

#[repr(C)]
struct Model {
    model: Mat4x4f,
}

fn zip_basic_input_size(m: &BasicMaterialFace, mesh: &Mesh) -> u64 {
    match m.shader_ex() {
        BasicMaterialShader::None => {
            mesh.vertices.iter().len() as u64 * std::mem::size_of::<BasicInput>() as u64
        }
        BasicMaterialShader::Color => {
            mesh.vertices.iter().len() as u64 * std::mem::size_of::<BasicInputC>() as u64
        }
        BasicMaterialShader::Texture => {
            mesh.vertices.iter().len() as u64 * std::mem::size_of::<BasicInputT>() as u64
        }
        BasicMaterialShader::ColorTexture => {
            mesh.vertices.iter().len() as u64 * std::mem::size_of::<BasicInputCT>() as u64
        }
    }
}

fn zip_basic_input(face: &BasicMaterialFace, mesh: &Mesh) -> Vec<u8> {
    let mut ret = Vec::new();
    match face.shader_ex() {
        BasicMaterialShader::None => {
            for a in mesh.vertices.iter() {
                let input = BasicInput { vertices: *a };
                ret.extend_from_slice(any_as_u8_slice(&input));
            }
        }
        BasicMaterialShader::Color => {
            for (a, b) in mesh
                .vertices
                .iter()
                .zip(mesh.coord_vec4f(MeshCoordType::Color).unwrap().iter())
            {
                let input = BasicInputC {
                    vertices: *a,
                    colors: *b,
                };
                ret.extend_from_slice(any_as_u8_slice(&input));
            }
        }
        BasicMaterialShader::Texture => {
            for (a, b) in mesh
                .vertices
                .iter()
                .zip(mesh.coord_vec2f(MeshCoordType::TexCoord).unwrap().iter())
            {
                let input = BasicInputT {
                    vertices: *a,
                    textcoord: *b,
                };
                ret.extend_from_slice(any_as_u8_slice(&input));
            }
        }
        BasicMaterialShader::ColorTexture => {
            for ((a, b), c) in mesh
                .vertices
                .iter()
                .zip(mesh.coord_vec4f(MeshCoordType::Color).unwrap().iter())
                .zip(mesh.coord_vec2f(MeshCoordType::TexCoord).unwrap().iter())
            {
                let input = BasicInputCT {
                    vertices: *a,
                    colors: *b,
                    texcoord: *c,
                };
                ret.extend_from_slice(any_as_u8_slice(&input));
            }
        }
    }
    ret
}

macro_rules! include_basic_shader {
    ($name: tt) => {
        (
            include_bytes!(concat!("../../compile_shaders/basic/", $name, ".vert")),
            include_bytes!(concat!("../../compile_shaders/basic/", $name, ".frag")),
        )
    };
}

pub fn forward_shader_source(
    shader: BasicMaterialShader,
    alpha_test: Option<f32>,
) -> (&'static [u8], &'static [u8]) {
    if alpha_test.is_none() {
        match shader {
            BasicMaterialShader::None => include_basic_shader!("forward"),
            BasicMaterialShader::Color => include_basic_shader!("forward_c"),
            BasicMaterialShader::Texture => include_basic_shader!("forward_t"),
            BasicMaterialShader::ColorTexture => include_basic_shader!("forward_ct"),
        }
    } else {
        match shader {
            BasicMaterialShader::None => include_basic_shader!("forward_a"),
            BasicMaterialShader::Color => include_basic_shader!("forward_ca"),
            BasicMaterialShader::Texture => include_basic_shader!("forward_ta"),
            BasicMaterialShader::ColorTexture => include_basic_shader!("forward_cta"),
        }
    }
}

struct MaterialGpuResource {
    pso: PipelineStateObject,
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    sampler: Option<wgpu::Sampler>,
}

struct BasicMaterialHardwareRendererInner {
    pipeline_pass: HashMap<MaterialId, MaterialGpuResource>,

    uniform_vp: wgpu::Buffer,

    vp_bind_group: Option<wgpu::BindGroup>,

    static_mesh_merger: StaticMeshMerger,
    dynamic_mesh_buffer: GpuInputMainBuffers,
    uniform_buffer: GpuInputUniformBuffers,

    bind_group_for_objects: Vec<wgpu::BindGroup>,
}

pub struct BasicMaterialHardwareRenderer {
    inner: Option<BasicMaterialHardwareRendererInner>,
}

impl BasicMaterialHardwareRenderer {
    pub fn new() -> Self {
        Self { inner: None }
    }
    pub fn prepare_material_pipeline(
        &mut self,
        gpu: &WGPUResource,
        material: &Material,
        fmt: wgpu::TextureFormat,
    ) {
        let label = self.label();
        let inner = self.inner.as_mut().unwrap();

        let mat = material.face_by::<BasicMaterialFace>();
        let entry = inner.pipeline_pass.entry(material.id());
        let pipe = entry.or_insert_with(|| {
            let shader = mat.shader_ex();
            let (vs_source, fs_source) = forward_shader_source(shader, material.alpha_test());
            let vs = wgpu::util::make_spirv(&vs_source);
            let fs = wgpu::util::make_spirv(&fs_source);
            let vs = wgpu::ShaderModuleDescriptor { label, source: vs };
            let fs = wgpu::ShaderModuleDescriptor { label, source: fs };

            let fs_target = match material.blend() {
                Some(blend) => FsTarget::new_with_blend(fmt, blend),
                None => FsTarget::new(fmt),
            };

            let primitive = material.primitive();

            let depth = wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: !material.is_transparent(),
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            };

            let pass = PipelineReflector::new(label, gpu.device())
                .add_vs(vs)
                .add_fs(fs, fs_target)
                .with_depth(depth)
                .build(primitive.clone())
                .unwrap();

            let pso = PipelineStateObject::new(gpu.context().alloc_pso());

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

            let sampler = if match mat.shader_ex() {
                BasicMaterialShader::Texture => true,
                BasicMaterialShader::ColorTexture => true,
                _ => false,
            } {
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
                layout: &pass.bind_group_layouts[1],
                entries: &entries,
            });

            gpu.context().inner().map_pso(pso.id(), Some(pass));

            MaterialGpuResource {
                pso,
                buffer: buf,
                sampler,
                bind_group,
            }
        });

        let pass = gpu.context().inner().get_pso(pipe.pso.id());

        if inner.vp_bind_group.is_none() {
            let bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("basic material"),
                layout: &pass.bind_group_layouts[0],
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

    fn render_material<'a, 'b>(
        &mut self,
        ctx: &mut MaterialRenderContext<'a, 'b>,
        objects: &[u64],
        material: &Material,
    ) {
        let face = material.face_by::<BasicMaterialFace>();
        let gpu = ctx.gpu;
        let label = self.label();

        self.prepare_material_pipeline(gpu, material, ctx.hint_fmt);

        let inner = self.inner.as_mut().unwrap();
        let mut mgr = inner.pipeline_pass.get(&material.id()).unwrap();
        let pipeline = ctx.gpu.context().inner().get_pso(mgr.pso.id());

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
                    layout: &pipeline.bind_group_layouts[2],
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
        pass.set_pipeline(&pipeline.pipeline);
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

    // zorder 8bits
    // shader 8bits
    // ----------
    // pso_id 32bits
    fn sort_key(&mut self, material: &Material, gpu: &WGPUResource) -> u64 {
        let shader_id = material.face().shader_id();

        (material.id().id() as u64) | (shader_id << 48)
    }
}

#[derive(Default)]
pub struct BasicMaterialRendererFactory {}

impl MaterialRendererFactory for BasicMaterialRendererFactory {
    fn new(&self) -> Box<dyn MaterialRenderer> {
        Box::new(BasicMaterialHardwareRenderer::new())
    }
}
