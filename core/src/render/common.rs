use std::{
    borrow::Borrow,
    cell::RefCell,
    collections::HashMap,
    num::NonZeroU64,
    ops::{Not, Range},
    rc::Rc,
};

use crate::backends::wgpu_backend::WGPUResource;

pub struct UniformBinder {
    pub set: u32,
    pub buffers: Vec<wgpu::Buffer>,
    pub group: wgpu::BindGroup,
}

pub struct UniformBinderBuilder<'a> {
    set: u32,
    label: Option<&'a str>,
    device: &'a wgpu::Device,
    buffers: Vec<wgpu::Buffer>,
}

impl<'a> UniformBinderBuilder<'a> {
    pub fn new(set: u32, label: Option<&'a str>, device: &'a wgpu::Device) -> Self {
        Self {
            set,
            label,
            device,
            buffers: Vec::new(),
        }
    }

    pub fn add_buffer_to<T>(mut self) -> Self {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: self.label,
            size: std::mem::size_of::<T>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.buffers.push(buffer);
        self
    }

    pub fn build(self, layout: &wgpu::BindGroupLayout) -> UniformBinder {
        let entries: Vec<wgpu::BindGroupEntry> = self
            .buffers
            .iter()
            .enumerate()
            .map(|(idx, v)| wgpu::BindGroupEntry {
                binding: idx as u32,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    offset: 0,
                    size: NonZeroU64::new(v.size()),
                    buffer: v,
                }),
            })
            .collect();

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: self.label,
            layout,
            entries: &entries,
        });
        UniformBinder {
            buffers: self.buffers,
            set: self.set,
            group: bind_group,
        }
    }
}

struct SharedBuffer {
    buf: wgpu::Buffer,
    cap: u32,
    current_offset: u32,
    used_size: u32,

    used_objects: u32,
}

impl SharedBuffer {
    pub fn rest(&self) -> u64 {
        (self.cap - self.current_offset) as u64
    }
}

type SharedBufferRef = Rc<RefCell<SharedBuffer>>;

struct SingleMeshMergerData {
    share_buffer: SharedBufferRef,
    range: Range<u64>,
}

const ALIGNMENT: u64 = 8;

impl SingleMeshMergerData {
    fn create(
        gpu: &WGPUResource,
        label: Option<&'static str>,
        size: u64,
        usage: wgpu::BufferUsages,
    ) -> SharedBufferRef {
        let buf = gpu.device().create_buffer(&wgpu::BufferDescriptor {
            label,
            size,
            usage: wgpu::BufferUsages::COPY_DST | usage,
            mapped_at_creation: false,
        });

        Rc::new(RefCell::new(SharedBuffer {
            buf,
            cap: size as u32,
            current_offset: 0,
            used_size: 0,
            used_objects: 0,
        }))
    }

    pub fn new_buffered(
        gpu: &WGPUResource,
        label: Option<&'static str>,
        usage: wgpu::BufferUsages,
        data: &[u8],
        small_buffer_allocator: &mut Option<SharedBufferRef>,
    ) -> Self {
        let (buf, range) = if data.len() < 1024 * 4 {
            // use small allocator
            let recreate_sba = if let Some(v) = small_buffer_allocator {
                let rest = v.as_ref().borrow().rest();
                rest < data.len() as u64
            } else {
                true
            };

            if recreate_sba {
                let buf = Self::create(gpu, label, 1024 * 1024 * 2, usage);
                *small_buffer_allocator = Some(buf);
            }
            let range = {
                let sba = small_buffer_allocator.as_mut().unwrap();
                let mut sba = sba.borrow_mut();
                let offset = sba.current_offset as u64;
                let end = offset + data.len() as u64;
                let alignment_end = (end + ALIGNMENT - 1) & (ALIGNMENT - 1).not();

                sba.current_offset = alignment_end as u32;
                sba.used_objects += 1;
                sba.used_size = (alignment_end - offset) as u32;

                gpu.queue().write_buffer(&sba.buf, offset, data);

                offset..end
            };

            (small_buffer_allocator.as_mut().unwrap().clone(), range)
        } else {
            let buf = Self::create(gpu, label, data.len() as u64, usage);
            let range = 0..(data.len() as u64);
            {
                let sb = &buf.as_ref().borrow().buf;
                gpu.queue().write_buffer(sb, 0, data);
            }
            (buf, range)
        };

        Self {
            share_buffer: buf,
            range,
        }
    }
}

pub struct StaticMeshMergerData {
    index: SingleMeshMergerData,
    vertex: SingleMeshMergerData,
    vertex_props: SingleMeshMergerData,
    version: u64,
}

pub trait BufferAccessor<'a> {
    fn buffer_slice(&self, id: u64, range: Range<u64>) -> Option<wgpu::BufferSlice<'a>>;
}

impl StaticMeshMergerData {}

pub struct StaticMeshMerger {
    objects: HashMap<u64, StaticMeshMergerData>,
    small_shared_vertex_buffer: Option<SharedBufferRef>,
    small_shared_vertex_props_buffer: Option<SharedBufferRef>,
    small_shared_index_buffer: Option<SharedBufferRef>,
    label: Option<&'static str>,
}

pub struct IndexStaticMesh<'a> {
    inner: &'a StaticMeshMerger,
}

impl<'a> BufferAccessor<'a> for IndexStaticMesh<'a> {
    fn buffer_slice(&self, id: u64, range: Range<u64>) -> Option<wgpu::BufferSlice<'a>> {
        let p = self.inner.objects.get(&id).unwrap();
        let b = p.index.share_buffer.as_ref().borrow();
        Some(unsafe { std::mem::transmute(b.buf.slice(range)) })
    }
}

pub struct VertexStaticMesh<'a> {
    inner: &'a StaticMeshMerger,
}

impl<'a> BufferAccessor<'a> for VertexStaticMesh<'a> {
    fn buffer_slice(&self, id: u64, range: Range<u64>) -> Option<wgpu::BufferSlice<'a>> {
        let p = self.inner.objects.get(&id).unwrap();
        let b = p.vertex.share_buffer.as_ref().borrow();
        Some(unsafe { std::mem::transmute(b.buf.slice(range)) })
    }
}

pub struct VertexPropsStaticMesh<'a> {
    inner: &'a StaticMeshMerger,
}

impl<'a> BufferAccessor<'a> for VertexPropsStaticMesh<'a> {
    fn buffer_slice(&self, id: u64, range: Range<u64>) -> Option<wgpu::BufferSlice<'a>> {
        let p = self.inner.objects.get(&id).unwrap();
        let b = p.vertex_props.share_buffer.as_ref().borrow();
        Some(unsafe { std::mem::transmute(b.buf.slice(range)) })
    }
}

impl StaticMeshMerger {
    pub fn new(label: Option<&'static str>) -> Self {
        Self {
            objects: HashMap::new(),
            small_shared_index_buffer: None,
            small_shared_vertex_props_buffer: None,
            small_shared_vertex_buffer: None,
            label,
        }
    }

    pub fn write_cached(
        &mut self,
        gpu: &WGPUResource,
        object_id: u64,
        version: u64,
        index_data: &[u8],
        vertex_data: &[u8],
        vertex_props_data: &[u8],
    ) -> (Range<u64>, Range<u64>, Range<u64>) {
        let need_create = self
            .objects
            .get(&object_id)
            .map(|v| v.version != version)
            .unwrap_or(true);

        if need_create {
            let index = SingleMeshMergerData::new_buffered(
                gpu,
                self.label,
                wgpu::BufferUsages::INDEX,
                index_data,
                &mut self.small_shared_index_buffer,
            );
            let vertex = SingleMeshMergerData::new_buffered(
                gpu,
                self.label,
                wgpu::BufferUsages::VERTEX,
                vertex_data,
                &mut self.small_shared_vertex_buffer,
            );

            let vertex_props = SingleMeshMergerData::new_buffered(
                gpu,
                self.label,
                wgpu::BufferUsages::VERTEX,
                vertex_props_data,
                &mut self.small_shared_vertex_props_buffer,
            );
            let smd = StaticMeshMergerData {
                index,
                vertex,
                vertex_props,
                version,
            };

            self.objects.insert(object_id, smd);
        }
        let o = self.objects.get(&object_id).unwrap();
        (
            o.index.range.clone(),
            o.vertex.range.clone(),
            o.vertex_props.range.clone(),
        )
    }

    pub fn recall(&mut self) {}

    pub fn index(&self) -> IndexStaticMesh {
        IndexStaticMesh { inner: self }
    }

    pub fn vertex(&self) -> VertexStaticMesh {
        VertexStaticMesh { inner: self }
    }

    pub fn vertex_props(&self) -> VertexPropsStaticMesh {
        VertexPropsStaticMesh { inner: self }
    }
}
