use std::{collections::HashMap, sync::Arc};

use wgpu::util::DeviceExt;

use crate::{
    backends::wgpu_backend::WGPUResource,
    material::{Material, MaterialId},
    mesh::Mesh,
    scene::{Camera, SceneStorage},
};

use super::{common::FramedCache, Pipeline, PipelinePassResource};

pub struct ObjectBuffer {
    pub index: Option<wgpu::Buffer>,
    pub vertex: wgpu::Buffer,
    pub vertex_properties: Option<wgpu::Buffer>,
}

impl ObjectBuffer {
    fn draw_inner<'a>(
        &'a self,
        mesh: &Mesh,
        pass: &mut wgpu::RenderPass<'a>,
        with_properties: bool,
    ) {
        pass.set_vertex_buffer(0, self.vertex.slice(..));
        if with_properties {
            if let Some(p) = &self.vertex_properties {
                pass.set_vertex_buffer(1, p.slice(..));
            }
        }

        let index_type_u32 = mesh.indices_is_u32().unwrap_or_default();

        if let Some(index) = &self.index {
            if index_type_u32 {
                pass.set_index_buffer(index.slice(..), wgpu::IndexFormat::Uint32);
            } else {
                pass.set_index_buffer(index.slice(..), wgpu::IndexFormat::Uint16);
            }
        }

        // index
        if self.index.is_some() {
            pass.draw_indexed(0..mesh.index_count().unwrap(), 0, 0..1);
        } else {
            pass.draw(0..mesh.vertex_count() as u32, 0..1);
        }
    }
    pub fn draw_no_properties<'a>(&'a self, mesh: &Mesh, pass: &mut wgpu::RenderPass<'a>) {
        self.draw_inner(mesh, pass, false)
    }
    pub fn draw<'a>(&'a self, mesh: &Mesh, pass: &mut wgpu::RenderPass<'a>) {
        self.draw_inner(mesh, pass, true)
    }
}

fn create_static_object_buffer(id: u64, mesh: &Mesh, device: &wgpu::Device) -> ObjectBuffer {
    let index = if let Some(index) = mesh.indices_view() {
        Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{} index buffer", id)),
                contents: index,
                usage: wgpu::BufferUsages::INDEX,
            }),
        )
    } else {
        None
    };

    let vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("{} vertex buffer", id)),
        contents: mesh.vertices_view().unwrap_or_default(),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let vertex_properties = if !mesh.properties_view().is_empty() {
        Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{} properties buffer", id)),
                contents: mesh.properties_view(),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        )
    } else {
        None
    };

    ObjectBuffer {
        index,
        vertex,
        vertex_properties,
    }
}

pub struct MeshBufferCollector {
    // static_object_buffers: FramedCache<u64, ObjectBuffer>,
    // small_static_object_buffers: StaticMeshMerger,
    dynamic_object_buffers: HashMap<u64, ObjectBuffer>,
}

impl MeshBufferCollector {
    pub fn new() -> Self {
        Self {
            dynamic_object_buffers: HashMap::new(),
        }
    }

    pub fn add(&mut self, c: &SceneStorage, object_id: u64, device: &wgpu::Device) {
        let obj = match c.get(&object_id) {
            Some(v) => v,
            None => return,
        };
        let obj = obj.o();
        let mesh = obj.geometry().mesh();

        // if obj.geometry().is_static() {
        //     self.inner.static_object_buffers.get_or(*id, |_| {
        //         create_static_object_buffer(*id, &mesh, engine.device())
        //     });
        // } else {
        self.dynamic_object_buffers
            .entry(object_id)
            .or_insert_with(|| create_static_object_buffer(object_id, &mesh, device));
        // }
    }

    pub fn get(&self, c: &SceneStorage, object_id: u64) -> Option<&ObjectBuffer> {
        self.dynamic_object_buffers.get(&object_id)
    }

    pub fn recall(&mut self) {
        self.dynamic_object_buffers.clear();
    }

    pub fn finish(&mut self) {}
}

pub struct MaterialGpuResource {
    pub material_bind_buffers: FramedCache<MaterialId, Vec<wgpu::Buffer>>,
    pub bind_groups: FramedCache<MaterialId, Vec<Option<wgpu::BindGroup>>>,

    pub pipeline: PipelinePassResource,
}

fn create_materia_buffer(material: &Material, gpu: &WGPUResource) -> wgpu::Buffer {
    let mat = material.face();
    let data = mat.material_data();
    gpu.new_wvp_buffer_from(Some("basic material buffer"), data)
}

pub struct MaterialBufferCollector {
    material_pipelines_cache: FramedCache<u64, MaterialGpuResource>,
}

impl MaterialBufferCollector {
    pub fn new() -> Self {
        Self {
            material_pipelines_cache: FramedCache::new(),
        }
    }

    pub fn add_pipeline<F: FnOnce() -> PipelinePassResource>(
        &mut self,
        material: &Material,
        gpu: &WGPUResource,
        create_pipeline: F,
    ) {
        let key = material.hash_key();
        let c = self.material_pipelines_cache.get_mut_or(key, |_| {
            let pipeline = create_pipeline();

            MaterialGpuResource {
                material_bind_buffers: FramedCache::new(),
                bind_groups: FramedCache::new(),
                pipeline,
            }
        });
        c.material_bind_buffers.get_or(material.id(), |_| {
            let mut res = vec![];
            for index in &c.pipeline.pass {
                res.push(create_materia_buffer(material, gpu));
            }
            res
        });
    }

    pub fn add_bind_group<
        F: FnOnce(&PipelinePassResource, &[wgpu::Buffer]) -> Vec<Option<wgpu::BindGroup>>,
    >(
        &mut self,
        material: &Material,
        create_bind_group: F,
    ) {
        let key = material.hash_key();
        let res = self.material_pipelines_cache.get_mut(&key).unwrap();
        res.bind_groups.get_or(material.id(), |id| {
            let buf = res.material_bind_buffers.get(id).unwrap();
            create_bind_group(&res.pipeline, &buf)
        });
    }

    pub fn get(&self, material: &Material, pass: usize) -> (&Pipeline, &[Option<wgpu::BindGroup>]) {
        let key = material.hash_key();
        let res = self.material_pipelines_cache.get(&key).unwrap();
        let pipeline = &res.pipeline.pass[pass];

        (pipeline, res.bind_groups.get(&material.id()).unwrap())
    }

    pub fn recall(&mut self) {
        self.material_pipelines_cache.recall();
    }
}

pub trait MaterialBufferInstantiation {
    fn create_pipeline(
        &self,
        material: &Material,
        global_layout: &wgpu::BindGroupLayout,
        gpu: &WGPUResource,
    ) -> PipelinePassResource;
    fn create_bind_group(
        &self,
        material: &Material,
        buffers: &[wgpu::Buffer],
        pipeline: &PipelinePassResource,
        device: &wgpu::Device,
    ) -> Vec<Option<wgpu::BindGroup>>;
}

pub struct MaterialBufferInstantCollector {
    r: Box<dyn MaterialBufferInstantiation>,
    c: MaterialBufferCollector,
}

impl MaterialBufferInstantCollector {
    pub fn new<I: MaterialBufferInstantiation + 'static>(i: I) -> Self {
        Self {
            r: Box::new(i),
            c: MaterialBufferCollector::new(),
        }
    }

    pub fn add_pipeline_and_copy_buffer(
        &mut self,
        material: &Material,
        global_layout: &wgpu::BindGroupLayout,
        gpu: &WGPUResource,
    ) {
        self.c.add_pipeline(material, gpu, || {
            self.r.create_pipeline(material, global_layout, gpu)
        });
    }

    pub fn add_bind_group(&mut self, material: &Material, device: &wgpu::Device) {
        self.c.add_bind_group(material, |pipeline, buffers| {
            self.r
                .create_bind_group(material, buffers, pipeline, device)
        });
    }

    pub fn get(&self, material: &Material) -> (&Pipeline, &[Option<wgpu::BindGroup>]) {
        self.c.get(material, 0)
    }
    pub fn get_pass(
        &self,
        material: &Material,
        pass: usize,
    ) -> (&Pipeline, &[Option<wgpu::BindGroup>]) {
        self.c.get(material, pass)
    }

    pub fn recall(&mut self) {
        self.c.recall();
    }
}
