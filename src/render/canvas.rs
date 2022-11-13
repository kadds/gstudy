use crate::{backends::wgpu_backend::WGPUResource, types};
use std::{
    mem::size_of_val,
    num::NonZeroU32,
    sync::{
        atomic::{AtomicBool, AtomicPtr, AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};
use types::to_rgba_u8;
type Size = types::Point2<u32>;
type Color = types::Vec4f;
type Position2 = types::Point2<u32>;

#[derive(Debug, Clone)]
struct MatBuffer {
    size: [f32; 2],
}

struct CanvasInner {
    texture: wgpu::Texture,
    texture_view_list: Vec<(wgpu::TextureView, wgpu::BindGroup)>,
}

pub struct Canvas {
    inner: AtomicPtr<CanvasInner>,
    size: Size,

    // rgba normalize u8
    data: Box<[u8]>,
    update_state: Arc<AtomicU32>,
    write_index: AtomicU32,
    read_index: AtomicU32,
    download_state: Arc<AtomicU32>,
}

pub struct CanvasWriter<'a> {
    size: Size,
    data: &'a [u8],
    dirty_flag: &'a AtomicBool,
}

impl Drop for Canvas {
    fn drop(&mut self) {
        // for texture in &self.textures {
        //     let texture = texture.load(Ordering::SeqCst);
        //     if !texture.is_null() {
        //         unsafe {
        //             drop(Box::from_raw(texture));
        //         }
        //     }
        // }
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

impl<'a> Drop for CanvasWriter<'a> {
    fn drop(&mut self) {}
}

impl Canvas {
    pub fn new(size: Size) -> Arc<Self> {
        let mut vec = Vec::new();
        vec.resize(size.x as usize * size.y as usize * 4, 0);
        for iter in vec.chunks_exact_mut(4) {
            // alpha channel => 1.0
            iter[3] = 255;
        }
        let this = Self {
            inner: AtomicPtr::default(),
            size,
            data: vec.into_boxed_slice(),
            update_state: AtomicU32::new(1).into(),
            write_index: AtomicU32::new(1),
            read_index: AtomicU32::new(0),
            download_state: AtomicU32::new(0).into(),
        }
        .into();
        this
    }

    // pub fn writer<'a>(&'a self) -> CanvasWriter<'a> {
    //     CanvasWriter {
    //         data: &self.data,
    //         size: self.size,
    //         dirty_flag: &self.dirty_flag,
    //     }
    // }

    pub fn make_sure(&self, gpu: &WGPUResource, encoder: &mut wgpu::CommandEncoder) {
        self.prepare_texture(gpu, encoder);
    }

    pub fn display_frame(&self, gpu: &WGPUResource) -> Option<&wgpu::BindGroup> {
        let ptr = self.inner.load(Ordering::Relaxed);
        if ptr.is_null() {
            return None;
        }
        let write_index = self.write_index.load(Ordering::Relaxed);
        let read_index = (write_index + 2) % 3;

        gpu.queue().on_submitted_work_done(|| {});

        Some(unsafe { &(*ptr).texture_view_list[read_index as usize].1 })
    }

    pub fn writer_frame(&self) -> Option<&wgpu::TextureView> {
        if self.update_state.load(Ordering::Acquire) != 0 {
            return None;
        }

        let ptr = self.inner.load(Ordering::Relaxed);
        if ptr.is_null() {
            return None;
        }

        let write_index = self.write_index.load(Ordering::Relaxed);
        self.write_index
            .store((write_index + 1) % 3, Ordering::Relaxed);

        Some(unsafe { &(*ptr).texture_view_list[write_index as usize].0 })
    }

    fn prepare_texture(&self, gpu: &WGPUResource, encoder: &mut wgpu::CommandEncoder) {
        if self.download_state.load(Ordering::Relaxed) == 1 {
            self.download_texture(gpu.device(), gpu.queue(), encoder);
        }

        if self.inner.load(Ordering::Relaxed).is_null() {
            let device = gpu.device();
            let (texture, texture_views) = Self::new_texture(device, self.size);

            let texture_bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("canvas bind group layout"),
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

            let views = texture_views
                .into_iter()
                .enumerate()
                .map(|(idx, view)| {
                    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("canvas bind group"),
                        layout: &texture_bind_group_layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&view),
                        }],
                    });
                    (view, bind_group)
                })
                .collect();

            let box_inner = Box::new(CanvasInner {
                texture,
                texture_view_list: views,
            });

            self.inner
                .store(Box::into_raw(box_inner), Ordering::Relaxed);
            self.flush_texture(gpu, 0, true, encoder);
        }
    }

    fn flush_texture(
        &self,
        gpu: &WGPUResource,
        idx: usize,
        force: bool,
        encoder: &wgpu::CommandEncoder,
    ) {
        let flag = self.update_state.load(Ordering::Acquire);
        if flag != 0 || force {
            let ptr = self.inner.load(Ordering::Relaxed);
            if ptr.is_null() {
                return;
            }
            let wgpu_texture = unsafe { &(*ptr).texture };
            let queue = gpu.queue();
            self.upload_texture(queue, wgpu_texture, idx, encoder);
        }
    }

    fn new_texture(device: &wgpu::Device, size: Size) -> (wgpu::Texture, Vec<wgpu::TextureView>) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("canvas texture"),
            size: wgpu::Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 3,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
        });
        let texture_views = (0..3)
            .map(|i| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("canvas texture view"),
                    base_array_layer: i,
                    array_layer_count: NonZeroU32::new(1),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    ..Default::default()
                })
            })
            .collect();

        (texture, texture_views)
    }

    fn upload_texture(
        &self,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        idx: usize,
        encoder: &wgpu::CommandEncoder,
    ) {
        let size = self.size;
        let dst = wgpu::ImageCopyTexture {
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            texture,
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
        let state = self.update_state.clone();
        state.store(2, Ordering::SeqCst);
        queue.on_submitted_work_done(move || {
            if state
                .compare_exchange(2, 0, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
            } else {
                let _ = state.compare_exchange(3, 0, Ordering::SeqCst, Ordering::SeqCst);
            }
        });
    }

    pub fn request_download_texture(&self) -> Option<()> {
        if self
            .download_state
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return None;
        }
        Some(())
    }

    pub fn download_ok(&self) -> Option<bool> {
        let state = self.download_state.load(Ordering::Acquire);
        if state == 0 {
            None
        } else {
            if state == 3 {
                Some(true)
            } else {
                Some(false)
            }
        }
    }

    pub fn clean_download_state(&self) {
        self.download_state.store(0, Ordering::Release);
    }

    fn download_texture(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let buf = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("download texture"),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            size: (self.size.x * self.size.y * 4) as u64,
            mapped_at_creation: false,
        }));

        let ptr = self.inner.load(Ordering::Relaxed);
        if ptr.is_null() {
            return;
        }
        let write_index = self.write_index.load(Ordering::Relaxed);
        let read_index = (write_index + 2) % 3;
        let texture = unsafe { &(*ptr).texture };

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                texture,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &buf,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: NonZeroU32::new(self.size.x as u32 * 4),
                    rows_per_image: NonZeroU32::new(self.size.y as u32),
                },
            },
            wgpu::Extent3d {
                width: self.size.x,
                height: self.size.y,
                depth_or_array_layers: 1,
            },
        );
        let data_ptr = self.data.as_ptr();
        let size = self.data.len();
        let data = unsafe { std::slice::from_raw_parts_mut(data_ptr as *mut u8, size) };
        self.download_state.store(2, Ordering::SeqCst);
        let state = self.download_state.clone();

        queue.on_submitted_work_done(move || {
            let buf_copy = buf.clone();
            buf.slice(..)
                .map_async(wgpu::MapMode::Read, move |callback| {
                    if let Err(err) = callback {
                        log::error!("download {}", err);
                    } else {
                        std::thread::spawn(move || {
                            data.copy_from_slice(&buf_copy.slice(..).get_mapped_range());
                            std::thread::sleep(Duration::from_millis(500));
                            state.store(3, Ordering::SeqCst);
                            buf_copy.unmap();
                        });
                    }
                });
        });
    }

    pub fn texture_data(&self) -> &[u8] {
        &self.data
    }
    pub fn size(&self) -> Size {
        self.size
    }
}
