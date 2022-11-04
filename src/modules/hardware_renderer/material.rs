use std::collections::{HashMap, HashSet};

use wgpu::util::{BufferInitDescriptor, DeviceExt};

use crate::{
    backends::wgpu_backend::{PassEncoder, WGPUResource},
    modules::hardware_renderer::common::FsTarget,
    render::{material::BasicMaterial, scene::Object, Camera, Material, Scene},
    util,
};

use super::common::PipelinePass;

#[derive(Debug)]
pub struct MaterialRenderContext<'a, 'b> {
    pub gpu: &'a WGPUResource,
    pub camera: &'a Camera,
    pub scene: &'a Scene,
    pub encoder: &'a mut PassEncoder<'b>,
}

pub trait MaterialRenderer<T: Material> {
    fn render<'a, 'b>(&mut self, ctx: &mut MaterialRenderContext<'a, 'b>, objects: &HashSet<u64>);
}

pub struct MaterialHardwareRenderer {}

struct BufferCache {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    mvp: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

pub struct BasicMaterialHardwareRendererInner {
    pipeline_pass: Option<PipelinePass>,
    buffer_cache: HashMap<u64, Box<BufferCache>>,
}

pub struct BasicMaterialHardwareRenderer {
    inner: BasicMaterialHardwareRendererInner,
}

impl BasicMaterialHardwareRenderer {
    pub fn new() -> Self {
        Self {
            inner: BasicMaterialHardwareRendererInner {
                pipeline_pass: None,
                buffer_cache: HashMap::new(),
            },
        }
    }
    pub fn prepare_pipeline(&mut self, device: &wgpu::Device) {
        let label = self.label();
        let inner = &mut self.inner;
        if inner.pipeline_pass.is_none() {
            let vs = wgpu::include_spirv!("../../compile_shaders/material/basic/forward.vert");
            let fs = wgpu::include_spirv!("../../compile_shaders/material/basic/forward.frag");
            let pipeline_info = super::common::PipelineReflector::new(label, device)
                .add_vs(vs)
                .add_fs(fs, FsTarget::new(wgpu::TextureFormat::Rgba8Unorm))
                .build(wgpu::PrimitiveState::default());
            inner.pipeline_pass = Some(pipeline_info);
        }
    }
    pub fn label(&self) -> Option<&'static str> {
        Some("basic material")
    }
}

impl MaterialRenderer<BasicMaterial> for BasicMaterialHardwareRenderer {
    fn render<'a, 'b>(&mut self, ctx: &mut MaterialRenderContext<'a, 'b>, objects: &HashSet<u64>) {
        let device = ctx.gpu.device();
        self.prepare_pipeline(device);
        let label = self.label();
        let inner = &mut self.inner;
        let pipeline = inner.pipeline_pass.as_ref().unwrap();
        let mut pass = ctx.encoder.new_pass();
        pass.set_pipeline(&pipeline.pipeline);
        // render it
        let device = ctx.gpu.device();
        for id in objects {
            let object = ctx.scene.get_object(*id).unwrap();
            let geo = object.geometry();
            let mesh_texture = geo.mesh_texture();
            let mesh = mesh_texture.mesh;
            inner.buffer_cache.entry(*id).or_insert_with(|| {
                let vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
                    label,
                    contents: crate::util::any_as_u8_slice_array(&mesh.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
                    label,
                    contents: crate::util::any_as_u8_slice_array(&mesh.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
                let mvp_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label,
                    size: 4 * 4 * 4,
                    usage: wgpu::BufferUsages::VERTEX
                        | wgpu::BufferUsages::UNIFORM
                        | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label,
                    layout: &pipeline.bind_group_layouts[0],
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &mvp_buffer,
                            offset: 0,
                            size: None,
                        }),
                    }],
                });
                BufferCache {
                    vertex: vertex_buffer,
                    index: index_buffer,
                    mvp: mvp_buffer,
                    bind_group,
                }
                .into()
            });
        }

        for id in objects {
            let c = inner.buffer_cache.get(id).unwrap();
            let object = ctx.scene.get_object(*id).unwrap();
            let geo = object.geometry();
            let mesh_texture = geo.mesh_texture();
            let mesh = mesh_texture.mesh;
            let vp = ctx.camera.mat_vp();
            let mvp = vp.0 * vp.1;

            pass.set_vertex_buffer(0, c.vertex.slice(..));
            pass.set_index_buffer(c.index.slice(..), wgpu::IndexFormat::Uint32);
            ctx.gpu
                .queue()
                .write_buffer(&c.mvp, 0, util::any_as_u8_slice_array(mvp.as_slice()));
            pass.set_bind_group(0, &c.bind_group, &[]);
            pass.draw_indexed(0..mesh.indices.len() as u32, 0, 0..1);
        }
    }
}
