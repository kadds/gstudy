use std::{collections::{HashMap, HashSet}, sync::Arc, any::Any};

use wgpu::util::{BufferInitDescriptor, DeviceExt};

use crate::{
    backends::wgpu_backend::{PassEncoder, WGPUResource},
    modules::hardware_renderer::common::FsTarget,
    render::{material::{DepthMaterial, downcast, BasicMaterial}, scene::Object, Camera, Material, Scene},
    util::{self, any_as_u8_slice_array}, geometry::Mesh, types::{Vec3f, Vec4f, Vec2f},
};

use super::common::PipelinePass;

#[derive(Debug)]
pub struct MaterialRenderContext<'a, 'b> {
    pub gpu: &'a WGPUResource,
    pub camera: &'a Camera,
    pub scene: &'a Scene,
    pub encoder: &'a mut PassEncoder<'b>,
}

pub trait MaterialRenderer: Send {
    fn render_material<'a, 'b>(&mut self, ctx: &mut MaterialRenderContext<'a, 'b>, objects: &Vec<u64>, material: &dyn Material);
}

struct BufferCache {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    mvp: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl BufferCache {
    pub fn make_mvp(label: Option<&str>, device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> (wgpu::Buffer, wgpu::BindGroup) {
        let mvp = device.create_buffer(&wgpu::BufferDescriptor {
            label,
            size: 4 * 4 * 4,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::UNIFORM
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label,
            layout: layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &mvp,
                    offset: 0,
                    size: None,
                }),
            }],
        });
        (mvp, bind_group)
    }
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

pub struct BasicMaterialHardwareRendererInner {
    pipeline_pass: HashMap<u64, PipelinePass>,
    buffer_cache: HashMap<u64, Box<BufferCache>>,
}

pub struct BasicMaterialHardwareRenderer {
    inner: BasicMaterialHardwareRendererInner,
}

impl BasicMaterialHardwareRenderer {
    pub fn new() -> Self {
        Self {
            inner: BasicMaterialHardwareRendererInner {
                pipeline_pass: HashMap::new(),
                buffer_cache: HashMap::new(),
            },
        }
    }
    pub fn prepare_pipeline(&mut self, device: &wgpu::Device, material: &BasicMaterial) {
        let label = self.label();
        let inner = &mut self.inner;
        let entry = inner.pipeline_pass.entry(material.material_id().unwrap());
        entry.or_insert_with(|| {
            let p = material.inner();
            let mut ps = wgpu::PrimitiveState::default();
            if p.line {
                ps.topology = wgpu::PrimitiveTopology::LineList;
            }
            let (vs, fs) = if material.inner().has_color {
                let vs = wgpu::include_spirv!("../../compile_shaders/material/basic/forward_c.vert");
                let fs = wgpu::include_spirv!("../../compile_shaders/material/basic/forward_c.frag");
                (vs, fs)
            } else {
                let vs = wgpu::include_spirv!("../../compile_shaders/material/basic/forward.vert");
                let fs = wgpu::include_spirv!("../../compile_shaders/material/basic/forward.frag");
                (vs, fs)
            };
            super::common::PipelineReflector::new(label, device)
                .add_vs(vs)
                .add_fs(fs, FsTarget::new(wgpu::TextureFormat::Rgba8Unorm))
                .build(ps)
        });
    }
    pub fn label(&self) -> Option<&'static str> {
        Some("basic material")
    }
}

impl MaterialRenderer for BasicMaterialHardwareRenderer {
    fn render_material<'a, 'b>(&mut self, ctx: &mut MaterialRenderContext<'a, 'b>, objects: &Vec<u64>, material: &dyn Material) {
        let material: &BasicMaterial = downcast(material);
        let device = ctx.gpu.device();
        self.prepare_pipeline(device, &material);

        let label = self.label();
        let inner = &mut self.inner;
        let pipeline = inner.pipeline_pass.get(&material.material_id().unwrap()).unwrap();
        let mut pass = ctx.encoder.new_pass();
        pass.set_pipeline(&pipeline.pipeline);
        // render it
        let device = ctx.gpu.device();
        for id in objects {
            let object = ctx.scene.get_object(*id).unwrap();
            let geo = object.geometry();
            let mesh = geo.mesh();
            inner.buffer_cache.entry(*id).or_insert_with(|| {

                let (mvp, bind_group) = BufferCache::make_mvp(label, device, &pipeline.bind_group_layouts[0]);
                let index = device.create_buffer_init(&BufferInitDescriptor { label, contents: any_as_u8_slice_array(&mesh.indices), usage: wgpu::BufferUsages::INDEX });
                let mut vertex_data: Vec<BasicInputC> = mesh.vertices.iter().zip(mesh.vertices_color.as_ref().unwrap().iter()).map(|(a, b)|
            BasicInputC{vertices: *a, colors: *b}).collect();

                let vertex = device.create_buffer_init(&BufferInitDescriptor { label, contents: any_as_u8_slice_array(&vertex_data), usage: wgpu::BufferUsages::VERTEX});

                BufferCache {vertex, index, mvp, bind_group }.into()
            });
        }

        for id in objects {
            let c = inner.buffer_cache.get(id).unwrap();
            let object = ctx.scene.get_object(*id).unwrap();
            let geo = object.geometry();
            let mesh = geo.mesh();
            let vp = ctx.camera.vp();

            pass.set_vertex_buffer(0, c.vertex.slice(..));
            pass.set_index_buffer(c.index.slice(..), wgpu::IndexFormat::Uint32);
            ctx.gpu
                .queue()
                .write_buffer(&c.mvp, 0, util::any_as_u8_slice_array(vp.as_slice()));
            pass.set_bind_group(0, &c.bind_group, &[]);
            pass.draw_indexed(0..mesh.indices.len() as u32, 0, 0..1);
        }
    }
}


pub struct DepthMaterialHardwareRendererInner {
    pipeline_pass: HashMap<u64, PipelinePass>,
    buffer_cache: HashMap<u64, Box<BufferCache>>,
}

pub struct DepthMaterialHardwareRenderer {
    inner: DepthMaterialHardwareRendererInner,
}

impl DepthMaterialHardwareRenderer {
    pub fn new() -> Self {
        Self {
            inner: DepthMaterialHardwareRendererInner {
                pipeline_pass: HashMap::new(),
                buffer_cache: HashMap::new(),
            },
        }
    }
    pub fn prepare_pipeline(&mut self, device: &wgpu::Device, material: &DepthMaterial) {
        let label = self.label();
        let inner = &mut self.inner;
        let entry = inner.pipeline_pass.entry(material.material_id().unwrap());
        entry.or_insert_with(|| {
            let p = material.inner();
            let mut ps = wgpu::PrimitiveState::default();
            if p.line {
                ps.topology = wgpu::PrimitiveTopology::LineList;
            }
            let vs = wgpu::include_spirv!("../../compile_shaders/material/depth/depth.vert");
            let fs = wgpu::include_spirv!("../../compile_shaders/material/depth/depth.frag");
            super::common::PipelineReflector::new(label, device)
                .add_vs(vs)
                .add_fs(fs, FsTarget::new(wgpu::TextureFormat::Rgba8Unorm))
                .build(ps)
        });
    }

    pub fn label(&self) -> Option<&'static str> {
        Some("depth material")
    }
}

impl MaterialRenderer for DepthMaterialHardwareRenderer {
    fn render_material<'a, 'b>(&mut self, ctx: &mut MaterialRenderContext<'a, 'b>, objects: &Vec<u64>, material: &dyn Material) {
        let material: &DepthMaterial = downcast(material);
        let device = ctx.gpu.device();
        self.prepare_pipeline(device, &material);

        let label = self.label();
        let inner = &mut self.inner;
        let pipeline = inner.pipeline_pass.get(&material.material_id().unwrap()).unwrap();

        let mut pass = ctx.encoder.new_pass();
        pass.set_pipeline(&pipeline.pipeline);
        // render it
        let device = ctx.gpu.device();
        for id in objects {
            let object = ctx.scene.get_object(*id).unwrap();
            let geo = object.geometry();
            let mesh = geo.mesh();
            inner.buffer_cache.entry(*id).or_insert_with(|| {
                let (mvp, bind_group) = BufferCache::make_mvp(label, device, &pipeline.bind_group_layouts[0]);
                let index = device.create_buffer_init(&BufferInitDescriptor { label, contents: any_as_u8_slice_array(&mesh.indices), usage: wgpu::BufferUsages::INDEX });
                let vertex = device.create_buffer_init(&BufferInitDescriptor { label, contents: any_as_u8_slice_array(&mesh.vertices), usage: wgpu::BufferUsages::VERTEX });
                BufferCache {vertex, index, mvp, bind_group }.into()
            });
        }

        for id in objects {
            let c = inner.buffer_cache.get(id).unwrap();
            let object = ctx.scene.get_object(*id).unwrap();
            let geo = object.geometry();
            let mesh = geo.mesh();
            let vp = ctx.camera.vp();

            pass.set_vertex_buffer(0, c.vertex.slice(..));
            pass.set_index_buffer(c.index.slice(..), wgpu::IndexFormat::Uint32);
            ctx.gpu
                .queue()
                .write_buffer(&c.mvp, 0, util::any_as_u8_slice_array(vp.as_slice()));
            pass.set_bind_group(0, &c.bind_group, &[]);
            pass.draw_indexed(0..mesh.indices.len() as u32, 0, 0..1);
        }
    }
}