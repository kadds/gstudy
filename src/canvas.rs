use crate::{renderer::RenderContext, types::*};
use std::{
    cell::UnsafeCell,
    num::NonZeroU32,
    sync::{atomic::AtomicPtr, Arc, Mutex},
};

#[derive(Debug, Clone)]
struct MatBuffer {
    size: [f32; 2],
}

struct Inner {
    pub texture: (wgpu::Texture, wgpu::BindGroup),
    pub size: Size,
}

struct BufData {
    pub size: Size,
    pub buf: Box<[u8]>,
    pub drity_flag: bool,
    pub stopped: bool,
}

impl BufData {
    pub fn new(size: Size) -> Self {
        let mut vec = Vec::new();
        vec.resize(size.width as usize * size.height as usize * 4, 0);
        for iter in vec.chunks_exact_mut(4) {
            // alpha channel => 1.0
            iter[0] = 255;
        }
        Self {
            size,
            buf: vec.into_boxed_slice(),
            drity_flag: true,
            stopped: false,
        }
    }
}

pub struct Canvas {
    inner: Option<Inner>,
    data: Arc<UnsafeCell<BufData>>,
}

pub struct Writer {
    data: Arc<UnsafeCell<BufData>>,
}

impl Canvas {
    pub fn new(size: Size) -> Self {
        Self {
            inner: None,
            data: Arc::new(UnsafeCell::new(BufData::new(size).into())),
        }
    }

    pub fn writer(&self) -> Writer {
        Writer {
            data: self.data.clone(),
        }
    }

    pub fn build_texture(&mut self, mut ctx: RenderContext) {
        let data = unsafe { self.data.get().as_mut().unwrap() };
        let new_size = data.size;
        let mut need_create = false;

        match self.inner.as_mut() {
            Some(v) => {
                if v.size != new_size {
                    need_create = true;
                }
            }
            None => {
                need_create = true;
            }
        };

        if need_create {
            let texture_bind_group_layout =
                ctx.device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            let texture = self.new_texture(ctx.device, new_size, &texture_bind_group_layout);
            self.inner = Some(Inner {
                texture,
                size: new_size,
            });
        }
        let mut drity = false;

        if data.drity_flag || need_create {
            drity = true;
        }
        if drity {
            let inner = self.inner.as_ref().unwrap();
            self.update_texture(&mut ctx, &inner.texture.0);
            data.drity_flag = false;
        }
    }

    pub fn resize_pixels(&self, size: Size) {}

    pub fn get_texture<'s>(&'s self) -> (&'s wgpu::Texture, &'s wgpu::BindGroup) {
        let inner = self.inner.as_ref().unwrap();
        (&inner.texture.0, &inner.texture.1)
    }

    pub fn new_texture(
        &self,
        device: &mut wgpu::Device,
        size: Size,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> (wgpu::Texture, wgpu::BindGroup) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
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
        (texture, bind_group)
    }
    fn update_texture(&self, ctx: &mut RenderContext<'_>, texture: &wgpu::Texture) {
        let data = unsafe { self.data.get().as_mut().unwrap() };
        let size = data.size;
        let dst = wgpu::ImageCopyTexture {
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            texture: &texture,
            aspect: wgpu::TextureAspect::All,
        };
        let data_layout = wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: NonZeroU32::new(size.width as u32 * 4),
            rows_per_image: NonZeroU32::new(size.height as u32),
        };
        // copy texture data
        ctx.queue.write_texture(
            dst,
            &data.buf,
            data_layout,
            wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
        );
    }
}

impl Writer {
    pub fn clear(&self, color: Color) {
        let data = unsafe { self.data.get().as_mut().unwrap() };
        let buf = &mut data.buf;
        for t in buf.chunks_exact_mut(4) {
            let rgba = color.to_rgba_u8();
            t.copy_from_slice(&rgba);
        }
    }

    pub fn draw_pixel(&self, pos: Position2, color: Color) {
        let data = unsafe { self.data.get().as_mut().unwrap() };
        let w = data.size.width;
        let buf = &mut data.buf;
        let mut iter = buf.chunks_mut(4).skip((pos.y * w + pos.x) as usize);
        iter.next().unwrap().copy_from_slice(&color.to_rgba_u8());
    }
}
