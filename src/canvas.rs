use crate::{
    renderer::{RenderContext, RenderObject, UpdateContext},
    types::*,
    util::*,
    UserEvent,
};
use std::{
    num::NonZeroU32,
    sync::{Arc, Mutex},
};
use winit::event::WindowEvent;
#[derive(Debug, Clone)]
struct MatBuffer {
    size: [f32; 2],
}

#[derive(Debug, Clone)]
struct Vertex {
    x: f32,
    y: f32,
    coord_x: f32,
    coord_y: f32,
}
impl Default for Vertex {
    fn default() -> Self {
        Vertex {
            x: 0f32,
            y: 0f32,
            coord_x: 0f32,
            coord_y: 0f32,
        }
    }
}
impl Vertex {
    pub fn new(x: f32, y: f32, coord_x: f32, coord_y: f32) -> Self {
        Self {
            x,
            y,
            coord_x,
            coord_y,
        }
    }
}

struct Inner {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group: wgpu::BindGroup,
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
    pub vertices: wgpu::Buffer,
    pub texture: (wgpu::Texture, wgpu::BindGroup),
    pub mat_buffer: wgpu::Buffer,
}

struct BufData {
    pub size: Size,
    pub buf: Box<[u8]>,
    pub drity_flag: bool,
}

impl BufData {
    pub fn new(size: Size) -> Self {
        let mut vec = Vec::new();
        vec.resize(size.width as usize * size.height as usize * 4, 0);
        Self {
            size,
            buf: vec.into_boxed_slice(),
            drity_flag: true,
        }
    }
}

pub struct Canvas {
    inner: Option<Inner>,
    data: Arc<Mutex<BufData>>,
    position: Rect,
    inner_size: Size,
    position_data: [Vertex; 6],
    position_changed: bool,
    size_changed: bool,
}

pub struct Writer {
    data: Arc<Mutex<BufData>>,
}

impl Canvas {
    pub fn new(size: Size) -> Self {
        Self {
            inner: None,
            data: Arc::new(BufData::new(size).into()),
            position: Rect::new(0, 0, 100, 100),
            inner_size: Size::new(1, 1),
            position_data: [
                Vertex::default(),
                Vertex::default(),
                Vertex::default(),
                Vertex::default(),
                Vertex::default(),
                Vertex::default(),
            ],
            size_changed: true,
            position_changed: true,
        }
    }

    pub fn writer(&self) -> Writer {
        Writer {
            data: self.data.clone(),
        }
    }

    // pub fn resize_pixels(&mut self, size: Size) {
    //     self.position.width = size.width;
    //     self.position.height = size.height;
    // }

    pub fn move_position(&mut self, rect: Rect) {
        self.position = rect;
        self.position_changed = true;
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
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
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
        let data = self.data.lock().unwrap();
        let size = data.size;
        let dst = wgpu::ImageCopyTexture {
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            texture: &texture,
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
        let mut data = self.data.lock().unwrap();
        let buf = &mut data.buf;
        for t in buf.chunks_mut(4) {
            let rgba = color.to_rgba_u8();
            t.copy_from_slice(&rgba);
        }
    }

    pub fn draw_pixel(&self, pos: Position2, color: Color) {
        let mut data = self.data.lock().unwrap();
        let w = data.size.width;
        let buf = &mut data.buf;
        let mut iter = buf.chunks_mut(4).skip((pos.y * w + pos.x) as usize);
        iter.next().unwrap().copy_from_slice(&color.to_rgba_u8());
    }
}

impl RenderObject for Canvas {
    fn zlevel(&self) -> i64 {
        1
    }

    fn render<'a>(&'a mut self, pass: &mut wgpu::RenderPass<'a>) {
        let inner = self.inner.as_mut().unwrap();
        pass.set_pipeline(&inner.pipeline);
        pass.set_bind_group(0, &inner.bind_group, &[]);
        pass.set_bind_group(1, &inner.texture.1, &[]);
        pass.set_vertex_buffer(0, inner.vertices.slice(..));
        pass.draw(0..6, 0..1);
    }

    fn on_user_event(&mut self, event: &UserEvent) {
        match event {
            &UserEvent::MoveCanvas(rect) => {
                self.move_position(rect);
            }
            _ => (),
        }
    }

    fn init_renderer(&mut self, device: &mut wgpu::Device) {
        let vs_source =
            device.create_shader_module(&wgpu::include_spirv!("compile_shaders/canvas.vert"));
        let fs_source =
            device.create_shader_module(&wgpu::include_spirv!("compile_shaders/canvas.frag"));
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Sampler {
                        comparison: false,
                        filtering: true,
                    },
                    count: None,
                },
            ],
        });
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                }],
            });

        let mat_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 8,
            usage: wgpu::BufferUsage::COPY_DST | wgpu::BufferUsage::UNIFORM,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: None,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0f32,
            lod_max_clamp: f32::MAX,
            compare: None,
            anisotropy_clamp: None,
            border_color: None,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &mat_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        let vertex_buffer_layout = [wgpu::VertexBufferLayout {
            array_stride: 4 * 4 as wgpu::BufferAddress,
            step_mode: wgpu::InputStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 4 * 2,
                    shader_location: 1,
                },
            ],
        }];

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &vs_source,
                entry_point: "main",
                buffers: &vertex_buffer_layout,
            },
            fragment: Some(wgpu::FragmentState {
                module: &fs_source,
                entry_point: "main",
                targets: &[wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrite::all(),
                }],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                clamp_depth: false,
                conservative: false,
                cull_mode: None,
                front_face: wgpu::FrontFace::default(),
                polygon_mode: wgpu::PolygonMode::default(),
                strip_index_format: None,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
        });

        let texture = self.new_texture(
            device,
            self.data.lock().unwrap().size,
            &texture_bind_group_layout,
        );

        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
            size: 4 * 4 * 6,
            mapped_at_creation: false,
        });

        self.inner = Some(Inner {
            pipeline,
            bind_group,
            texture_bind_group_layout,
            vertices,
            texture,
            mat_buffer,
        });
    }

    fn on_event(&mut self, event: &WindowEvent) {
        let mut new_size = None;
        match event {
            WindowEvent::Resized(size) => {
                new_size = Some(size);
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor: _,
                new_inner_size,
            } => {
                new_size = Some(new_inner_size);
            }
            _ => (),
        };
        if let Some(new_size) = new_size {
            self.size_changed = true;
            self.inner_size = Size::new(new_size.width as u32, new_size.height as u32);
        }
    }

    fn update<'a>(&'a mut self, _ctx: UpdateContext<'a>) -> bool {
        self.data.lock().unwrap().drity_flag
    }

    fn prepare_render<'a>(&'a mut self, mut ctx: crate::renderer::RenderContext<'a>) {
        let inner = self.inner.as_ref().unwrap();
        if self.data.lock().unwrap().drity_flag {
            self.update_texture(&mut ctx, &inner.texture.0);
        }
        if self.size_changed {
            let mat_buffer = MatBuffer {
                size: [self.inner_size.width as f32, self.inner_size.height as f32],
            };
            ctx.queue
                .write_buffer(&inner.mat_buffer, 0, any_as_u8_slice(&mat_buffer));
            self.size_changed = false;
        }
        if self.position_changed {
            let d = self.position_data.as_mut();
            let p = self.position;
            d[0] = Vertex::new(p.x as f32, p.y as f32, 0f32, 0f32);
            d[1] = Vertex::new(p.x as f32, p.bottom() as f32, 0f32, 1f32);
            d[2] = Vertex::new(p.right() as f32, p.bottom() as f32, 1f32, 1f32);
            d[3] = d[2].clone();
            d[4] = Vertex::new(p.right() as f32, p.y as f32, 1f32, 0f32);
            d[5] = d[0].clone();

            let data = any_as_u8_slice_array(&self.position_data);
            ctx.queue.write_buffer(&inner.vertices, 0, data);
            self.position_changed = false;
        }
    }
}
