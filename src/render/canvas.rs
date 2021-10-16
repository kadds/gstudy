use crate::{
    gpu_context::{GpuContext, GpuInstance, GpuInstanceRef},
    types,
    ui::RenderContext,
};
use std::{
    mem::size_of_val,
    num::NonZeroU32,
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicPtr, Ordering},
        Arc,
    },
};
use types::to_rgba_u8;
type Size = types::Point2<u32>;
type Color = types::Vec4f;
type Position2 = types::Point2<u32>;

#[derive(Debug, Clone)]
struct MatBuffer {
    size: [f32; 2],
}

pub struct Canvas {
    texture: AtomicPtr<(wgpu::Texture, wgpu::TextureView, wgpu::BindGroup)>,
    size: Size,
    // rgba normalize u8
    data: Box<[u8]>,
    dirty_flag: AtomicBool,
}

pub struct CanvasWriter<'a> {
    data: &'a [u8],
    size: Size,
    dirty_flag: &'a AtomicBool,
}

impl Drop for Canvas {
    fn drop(&mut self) {
        let texture = self.texture.load(Ordering::SeqCst);
        if !texture.is_null() {
            unsafe {
                Box::from_raw(texture);
            }
        }
    }
}

impl CanvasWriter<'_> {
    pub fn clear(&self, color: Color) {
        let data = unsafe { self.data.as_ptr() as *mut u8 };
        for i in 0..(self.size.x * self.size.y) as isize {
            let rgba = to_rgba_u8(&color);
            unsafe {
                data.offset(i * size_of_val(&rgba) as isize)
                    .copy_from_nonoverlapping(rgba.as_ptr(), size_of_val(&rgba));
            }
        }
    }

    pub fn draw_pixel(&self, pos: Position2, color: Color) {
        let data = unsafe { self.data.as_ptr() as *mut u8 };
        let rgba = to_rgba_u8(&color);
        unsafe {
            data.offset((pos.y * self.size.x + pos.x) as isize * size_of_val(&rgba) as isize)
                .copy_from_nonoverlapping(rgba.as_ptr(), size_of_val(&rgba));
        }
    }

    pub fn mark_dirty(&self) {
        self.dirty_flag.store(true, Ordering::SeqCst);
    }
}

impl Canvas {
    pub fn new(size: Size) -> Self {
        let mut vec = Vec::new();
        vec.resize(size.x as usize * size.y as usize * 4, 0);
        for iter in vec.chunks_exact_mut(4) {
            // alpha channel => 1.0
            iter[3] = 255;
        }
        Self {
            texture: AtomicPtr::new(ptr::null_mut()),
            size,
            data: vec.into_boxed_slice(),
            dirty_flag: AtomicBool::new(true),
        }
    }

    pub fn writer<'a>(&'a self) -> CanvasWriter<'a> {
        CanvasWriter {
            data: &self.data,
            size: self.size,
            dirty_flag: &self.dirty_flag,
        }
    }

    pub fn prepared(&self) -> bool {
        let t = self.texture.load(Ordering::Relaxed);
        !t.is_null()
    }

    pub fn build_texture(&self, gpu: &GpuInstance) {
        let mut need_create = false;
        if self.texture.load(Ordering::Relaxed).is_null() {
            need_create = true;
        }
        let device = gpu.device();
        if need_create {
            let texture_bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        },
                        count: None,
                    }],
                });
            let texture = self.new_texture(&device, self.size, &texture_bind_group_layout);
            let box_texture = Box::new(texture);

            self.texture
                .store(Box::into_raw(box_texture), Ordering::SeqCst);
        }
        let mut dirty_flag = self.dirty_flag.load(Ordering::Relaxed);

        if need_create {
            dirty_flag = true;
        }
        if dirty_flag {
            let texture = unsafe { &(*self.texture.load(Ordering::Relaxed)).0 };
            let queue = gpu.queue();
            self.update_texture(&queue, texture);
            self.dirty_flag.store(false, Ordering::SeqCst);
        }
    }

    pub fn get_texture<'s>(
        &'s self,
    ) -> (
        &'s wgpu::Texture,
        &'s wgpu::TextureView,
        &'s wgpu::BindGroup,
    ) {
        let t = self.texture.load(Ordering::Relaxed);
        return unsafe { (&(*t).0, &(*t).1, &(*t).2) };
    }

    pub fn new_texture(
        &self,
        device: &wgpu::Device,
        size: Size,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
        });
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: texture_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            }],
        });
        (texture, texture_view, bind_group)
    }

    fn update_texture(&self, queue: &wgpu::Queue, texture: &wgpu::Texture) {
        let size = self.size;
        let dst = wgpu::ImageCopyTexture {
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            texture: &texture,
            aspect: wgpu::TextureAspect::All,
        };
        let data_layout = wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: NonZeroU32::new(size.x as u32 * 4),
            rows_per_image: NonZeroU32::new(size.y as u32),
        };
        // copy texture data
        queue.write_texture(
            dst,
            &self.data,
            data_layout,
            wgpu::Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
        );
    }
}
