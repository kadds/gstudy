use wgpu::util::DeviceExt;

use crate::{
    mesh::{InstanceProperties, Mesh},
    scene::SceneStorage,
    cache::FramedCache,
};

pub struct ObjectBuffer {
    pub index: Option<wgpu::Buffer>,
    pub vertex: wgpu::Buffer,
    pub vertex_properties: Option<wgpu::Buffer>,
    pub instance_data: Option<wgpu::Buffer>,
    pub instance_count: u32,
    pub instance_version: u64,
}

impl ObjectBuffer {
    fn draw_inner<'a>(
        &'a self,
        mesh: &Mesh,
        pass: &mut wgpu::RenderPass<'a>,
        with_properties: bool,
    ) {
        pass.set_vertex_buffer(0, self.vertex.slice(..));
        let mut index = 1;
        if with_properties {
            if let Some(p) = &self.vertex_properties {
                pass.set_vertex_buffer(index, p.slice(..));
                index += 1;
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

        if let Some(instance) = &self.instance_data {
            pass.set_vertex_buffer(index, instance.slice(..));
            index += 1;
        }

        // index
        if self.index.is_some() {
            pass.draw_indexed(0..mesh.index_count().unwrap(), 0, 0..self.instance_count);
        } else {
            pass.draw(0..mesh.vertex_count() as u32, 0..self.instance_count);
        }
    }
    pub fn draw_no_properties<'a>(&'a self, mesh: &Mesh, pass: &mut wgpu::RenderPass<'a>) {
        self.draw_inner(mesh, pass, false)
    }
    pub fn draw<'a>(&'a self, mesh: &Mesh, pass: &mut wgpu::RenderPass<'a>) {
        self.draw_inner(mesh, pass, true)
    }
}

fn create_static_object_buffer(
    id: u64,
    mesh: &Mesh,
    instance: Option<&InstanceProperties>,
    device: &wgpu::Device,
) -> ObjectBuffer {
    profiling::scope!("static buffer", &format!("{}", id));
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

    let (instance_data, count) = if let Some(ins) = &instance {
        let view = ins.data.lock().unwrap();
        (
            Some(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{} instance buffer", id)),
                    contents: &view.view(),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
            ),
            view.count,
        )
    } else {
        (None, 1)
    };

    ObjectBuffer {
        index,
        vertex,
        vertex_properties,
        instance_data,
        instance_count: count as u32,
        instance_version: 0,
    }
}

fn update_dynamic_object_buffer(
    id: u64,
    mesh: &Mesh,
    instance: Option<&InstanceProperties>,
    device: &wgpu::Device,
    buf: &mut ObjectBuffer,
) {
    if let Some(view) = &instance {
        let v = view.data.lock().unwrap();
        if v.version != buf.instance_version {
            buf.instance_version = v.version;
            // copy buffer
            let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{} instance buffer", id)),
                contents: &v.view(),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
            buf.instance_data = Some(instance_buffer);
            buf.instance_count = v.count as u32;
        }
    }
}

fn create_dynamic_object_buffer(
    id: u64,
    mesh: &Mesh,
    instance: Option<&InstanceProperties>,
    device: &wgpu::Device,
) -> ObjectBuffer {
    profiling::scope!("dynamic buffer", &format!("{}", id));
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

    let (instance_data, count, version) = if let Some(ins) = &instance {
        let view = ins.data.lock().unwrap();
        (
            Some(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{} instance buffer", id)),
                    contents: &view.view(),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
            ),
            view.count,
            view.version,
        )
    } else {
        (None, 1, 0)
    };

    ObjectBuffer {
        index,
        vertex,
        vertex_properties,
        instance_data,
        instance_count: count as u32,
        instance_version: version,
    }
}

pub struct MeshBufferCollector {
    static_object_buffers: FramedCache<u64, ObjectBuffer>,
    // small_static_object_buffers: StaticMeshMerger,
    dynamic_object_buffers: FramedCache<u64, ObjectBuffer>,
}

impl MeshBufferCollector {
    pub fn new() -> Self {
        Self {
            static_object_buffers: FramedCache::new(),
            dynamic_object_buffers: FramedCache::new(),
        }
    }

    pub fn add(&mut self, c: &SceneStorage, object_id: u64, device: &wgpu::Device) {
        let obj = match c.get(&object_id) {
            Some(v) => v,
            None => return,
        };
        let obj = obj.o();
        let mesh = obj.geometry().mesh();
        let instance = obj.geometry().instance();

        if obj.geometry().info().is_static {
            self.static_object_buffers.get_or(object_id, |_| {
                create_static_object_buffer(object_id, &mesh, instance, device)
            });
        } else {
            let buf = self.dynamic_object_buffers.get_mut_or(object_id, |_| {
                create_dynamic_object_buffer(object_id, &mesh, instance, device)
            });
            update_dynamic_object_buffer(object_id, &mesh, instance, device, buf);
        }
    }

    pub fn get(&self, _c: &SceneStorage, object_id: u64) -> Option<&ObjectBuffer> {
        if let Some(v) = self.static_object_buffers.get(&object_id) {
            return Some(v);
        }
        self.dynamic_object_buffers.get(&object_id)
    }

    pub fn recall(&mut self) {
        self.static_object_buffers.recall();
        self.dynamic_object_buffers.recall();
    }

    pub fn finish(&mut self) {}
}

