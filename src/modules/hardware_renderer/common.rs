use std::{marker::PhantomData, num::NonZeroU64};

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
                    buffer: &v,
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

pub struct FrameUniformBufferSlice {
    buffer_index: usize,
    offset: u32,
}

pub struct FrameUniformBufferHolder<T> {
    buffers: Vec<wgpu::Buffer>,
    _pd: PhantomData<T>,
    chunk_count: usize,
}

impl<T> FrameUniformBufferHolder<T> {
    pub fn new(chunk_count: usize, device: &wgpu::Device) -> Self {
        let max_size = device.limits().max_uniform_buffer_binding_size;
        let alignment = device.limits().min_uniform_buffer_offset_alignment;

        Self {
            buffers: Vec::new(),
            _pd: PhantomData::default(),
            chunk_count,
        }
    }

    pub fn write_buffer(&mut self) -> FrameUniformBufferSlice {
        todo!()
    }

    pub fn recall(&mut self) {}

    pub fn finish(&mut self) {}
}
