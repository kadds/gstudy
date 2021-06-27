use crate::{
    maps,
    renderer::{RenderContext, UpdateContext},
    types::*,
    util::*,
    UserEvent,
};
use std::{collections::HashMap, num::NonZeroU32, time::Duration};

use egui::{CtxRef, RawInput};
use winit::event::WindowEvent;

use crate::renderer::RenderObject;

#[derive(Debug, Clone)]
struct MatBuffer {
    size: [f32; 2],
}

const DEFAULT_VERTEX_BUFFER_SIZE: usize = 1 << 18;
const DEFAULT_INDEX_BUFFER_SIZE: usize = 1 << 16;

#[derive(Debug)]
struct RenderStage {
    pub vertex_buffer: usize,
    pub index_buffer: usize,
    pub rect: Rect,
    pub count_idx: u32,
    pub base_vertex: u32,
    pub base_index: u32,
    pub texture_id: egui::TextureId,
}

struct Texture {
    pub texture: wgpu::Texture,
    pub bind_group: wgpu::BindGroup,
    pub size: Size,
    pub version: u64,
}

struct DynRenderState {
    textures: HashMap<egui::TextureId, Texture>,
    stages: Vec<RenderStage>,
    vertex_buffers: Vec<(wgpu::Buffer, usize)>,
    vertex_offset: usize,
    index_buffers: Vec<(wgpu::Buffer, usize)>,
    index_offset: usize,
}

struct Inner {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    mat_buffer: wgpu::Buffer,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    inner_size: Size,
    size_changed: bool,
    last_mouse_pos: (f32, f32),
    last_modifiers: egui::Modifiers,
    dyn_state: DynRenderState,
    meshes: Option<Vec<egui::ClippedMesh>>,
}

pub struct UI {
    ui_ctx: egui::CtxRef,
    update_fn: Box<dyn FnMut(egui::CtxRef, &UpdateContext)>,
    set_fn: Box<dyn FnMut(&egui::Output)>,
    input: RawInput,
    inner: Option<Inner>,
}

impl UI {
    pub fn new(
        update_fn: Box<dyn FnMut(egui::CtxRef, &UpdateContext)>,
        set_fn: Box<dyn FnMut(&egui::Output)>,
    ) -> Self {
        Self {
            ui_ctx: egui::CtxRef::default(),
            update_fn,
            set_fn,
            input: RawInput::default(),
            inner: None,
        }
    }
}

impl RenderObject for UI {
    fn zlevel(&self) -> i64 {
        0
    }

    fn update(&mut self, ctx: UpdateContext) -> bool {
        let inner = self.inner.as_mut().unwrap();
        let mut input = RawInput::default();
        std::mem::swap(&mut input, &mut self.input);
        let time: Duration = ctx.update_statistics.elapsed();
        input.time = Some(time.as_micros() as f64 / 1000_000f64);
        input.modifiers = inner.last_modifiers.clone();

        let ui_ctx = &mut self.ui_ctx;

        ui_ctx.begin_frame(input);
        (self.update_fn)(ui_ctx.clone(), &ctx);
        let (output, shapes) = ui_ctx.end_frame();
        let meshes = ui_ctx.tessellate(shapes);
        inner.meshes = Some(meshes);
        (self.set_fn)(&output);
        output.needs_repaint
    }

    fn prepare_render(&mut self, mut ctx: RenderContext) {
        let inner = self.inner.as_mut().unwrap();
        let state = &mut inner.dyn_state;
        state.new_frame();

        if inner.size_changed {
            let mat_buffer = MatBuffer {
                size: [
                    inner.inner_size.width as f32,
                    inner.inner_size.height as f32,
                ],
            };
            ctx.queue
                .write_buffer(&inner.mat_buffer, 0, any_as_u8_slice(&mat_buffer));
            inner.size_changed = false;
        }
        let meshes = inner.meshes.as_mut().unwrap();

        for mesh in meshes {
            let rect = mesh.0;
            let mesh = &mut mesh.1;
            let x = (rect.left() as u32).clamp(0, inner.inner_size.width);
            let y = (rect.top() as u32).clamp(0, inner.inner_size.height);
            let width = (rect.width() as u32).clamp(0, inner.inner_size.width - x);
            let height = (rect.height() as u32).clamp(0, inner.inner_size.height - y);

            if width == 0 || height == 0 {
                continue;
            }
            let rect = Rect::new(x, y, width, height);

            state.commit(
                &mut ctx,
                &mut mesh.indices,
                &mut mesh.vertices,
                rect,
                mesh.texture_id,
                &inner.texture_bind_group_layout,
                self.ui_ctx.clone(),
            );
        }
    }
    fn render<'a>(&'a mut self, pass: &mut wgpu::RenderPass<'a>) {
        let inner = self.inner.as_mut().unwrap();
        let state = &mut inner.dyn_state;
        pass.set_pipeline(&inner.pipeline);
        pass.set_bind_group(0, &inner.bind_group, &[]);
        for stage in &state.stages {
            let rect = stage.rect;
            pass.set_scissor_rect(rect.x, rect.y, rect.width, rect.height);

            let texture = state.textures.get(&stage.texture_id).unwrap();
            // bind texture
            pass.set_bind_group(1, &texture.bind_group, &[]);
            let vertex_buffer = state.get_vertex_buffer(stage.vertex_buffer);
            let index_buffer = state.get_index_buffer(stage.index_buffer);
            pass.set_vertex_buffer(0, vertex_buffer.slice((stage.base_vertex as u64)..));
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(
                (stage.base_index)..(stage.count_idx + stage.base_index),
                0,
                0..1,
            );
        }
    }

    fn init_renderer(&mut self, device: &mut wgpu::Device) {
        let vs_source =
            device.create_shader_module(&wgpu::include_spirv!("compile_shaders/ui.vert"));
        let fs_source =
            device.create_shader_module(&wgpu::include_spirv!("compile_shaders/ui.frag"));
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
            array_stride: 4 * 5 as wgpu::BufferAddress,
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
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32,
                    offset: 4 * 4,
                    shader_location: 2,
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

        self.inner = Some(Inner {
            pipeline,
            bind_group,
            mat_buffer,
            texture_bind_group_layout,
            dyn_state: DynRenderState::new(),
            inner_size: Size::new(0, 0),
            last_mouse_pos: (0f32, 0f32),
            last_modifiers: egui::Modifiers {
                alt: false,
                ctrl: false,
                shift: false,
                command: false,
                mac_cmd: false,
            },
            size_changed: true,
            meshes: None,
        });
    }

    fn on_user_event(&mut self, _event: &UserEvent) {}

    fn on_event(&mut self, event: &WindowEvent) {
        let inner = match self.inner.as_mut() {
            Some(inner) => inner,
            None => {
                return;
            }
        };
        match event {
            WindowEvent::Resized(size) => {
                inner.inner_size = Size::new(size.width, size.height);
                inner.size_changed = true;
                self.input.screen_rect = Some(egui::Rect {
                    min: egui::Pos2::new(0f32, 0f32),
                    max: egui::Pos2::new(size.width as f32, size.height as f32),
                })
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor: _,
                new_inner_size,
            } => {
                let size = new_inner_size;
                inner.inner_size = Size::new(size.width, size.height);
                inner.size_changed = true;
                self.input.screen_rect = Some(egui::Rect {
                    min: egui::Pos2::new(0f32, 0f32),
                    max: egui::Pos2::new(size.width as f32, size.height as f32),
                })
            }
            WindowEvent::ModifiersChanged(state) => {
                inner.last_modifiers.alt = state.alt();
                inner.last_modifiers.ctrl = state.ctrl();
                inner.last_modifiers.command = state.logo();
                inner.last_modifiers.shift = state.shift();
                inner.last_modifiers.mac_cmd = false;
                if cfg!(targetos = "macos") {
                    inner.last_modifiers.mac_cmd = inner.last_modifiers.command;
                } else {
                    inner.last_modifiers.command = inner.last_modifiers.ctrl;
                }
                // log::info!("{:?}", inner.last_modifiers);
            }
            WindowEvent::KeyboardInput {
                device_id: _,
                input,
                is_synthetic: _,
            } => {
                let key = match maps::match_egui_key(
                    input
                        .virtual_keycode
                        .unwrap_or(winit::event::VirtualKeyCode::Apostrophe),
                ) {
                    Some(k) => k,
                    None => {
                        return;
                    }
                };
                // log::info!("k {:?}", key);
                if key == egui::Key::C && inner.last_modifiers.command {
                    self.input.events.push(egui::Event::Copy);
                }
                if key == egui::Key::X && inner.last_modifiers.command {
                    self.input.events.push(egui::Event::Cut);
                }
                self.input.events.push(egui::Event::Key {
                    pressed: input.state == winit::event::ElementState::Pressed,
                    modifiers: inner.last_modifiers,
                    key,
                });
            }
            WindowEvent::MouseWheel {
                device_id: _,
                delta,
                phase: _,
                modifiers: _,
            } => {
                self.input.scroll_delta = match *delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        egui::Vec2::new(x * 50f32, y * 50f32)
                    }
                    winit::event::MouseScrollDelta::PixelDelta(a) => {
                        egui::Vec2::new(a.x as f32, a.y as f32)
                    }
                };
            }
            WindowEvent::CursorLeft { device_id: _ } => {
                self.input.events.push(egui::Event::PointerGone {});
            }
            WindowEvent::CursorMoved {
                device_id: _,
                position,
                modifiers: _,
            } => {
                self.input
                    .events
                    .push(egui::Event::PointerMoved(egui::Pos2::new(
                        position.x as f32,
                        position.y as f32,
                    )));
                inner.last_mouse_pos = (position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput {
                device_id: _,
                state,
                button,
                modifiers: _,
            } => {
                let button = match button {
                    winit::event::MouseButton::Left => egui::PointerButton::Primary,
                    winit::event::MouseButton::Right => egui::PointerButton::Secondary,
                    winit::event::MouseButton::Middle => egui::PointerButton::Middle,
                    winit::event::MouseButton::Other(_) => {
                        return;
                    }
                };
                let pressed = match state {
                    winit::event::ElementState::Pressed => true,
                    winit::event::ElementState::Released => false,
                };
                self.input.events.push(egui::Event::PointerButton {
                    pos: egui::pos2(inner.last_mouse_pos.0, inner.last_mouse_pos.1),
                    modifiers: inner.last_modifiers,
                    pressed,
                    button,
                });
            }
            WindowEvent::ReceivedCharacter(c) => {
                let c = *c;
                if !c.is_ascii_control() {
                    self.input.events.push(egui::Event::Text(c.to_string()));
                }
            }
            _ => (),
        };
    }
}

impl DynRenderState {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            vertex_buffers: Vec::new(),
            index_buffers: Vec::new(),
            stages: Vec::new(),
            vertex_offset: 0,
            index_offset: 0,
        }
    }

    pub fn new_frame(&mut self) {
        self.stages.clear();
        self.vertex_offset = 0;
        self.index_offset = 0;
        for (_, used) in &mut self.vertex_buffers {
            *used = 0;
        }
        for (_, used) in &mut self.index_buffers {
            *used = 0;
        }
    }

    pub fn commit(
        &mut self,
        ctx: &mut RenderContext<'_>,
        indices: &mut [u32],
        vertices: &mut [egui::epaint::Vertex],
        rect: Rect,
        texture_id: egui::TextureId,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        ui_ctx: CtxRef,
    ) {
        let mut index_cursor = 0;

        while index_cursor < indices.len() {
            self.prepare_index_buffer(ctx, self.index_offset);
            self.prepare_vertex_buffer(ctx, self.vertex_offset);

            let (index_buffer, iused) = self.index_buffers.get_mut(self.index_offset).unwrap();
            let (vertex_buffer, vused) = self.vertex_buffers.get_mut(self.vertex_offset).unwrap();

            let iremain = DEFAULT_INDEX_BUFFER_SIZE - *iused;
            let vremain = DEFAULT_VERTEX_BUFFER_SIZE - *vused;

            let span_start = index_cursor;
            let mut min_vindex = indices[index_cursor];
            let mut max_vindex = indices[index_cursor];

            while index_cursor < indices.len() {
                let (mut new_min, mut new_max) = (min_vindex, max_vindex);
                for i in 0..3 {
                    let idx = indices[index_cursor + i];
                    new_min = new_min.min(idx);
                    new_max = new_max.max(idx);
                }

                if new_max - new_min + 1 < vremain as u32 && index_cursor - span_start + 4 < iremain
                {
                    // Triangle fits
                    min_vindex = new_min;
                    max_vindex = new_max;
                    index_cursor += 3;
                } else {
                    break;
                }
            }

            assert!(
                index_cursor > span_start,
                "One triangle spanned more than {} vertices",
                DEFAULT_VERTEX_BUFFER_SIZE
            );
            let vertex_count = (max_vindex - min_vindex + 1) as usize;
            let index_count = index_cursor - span_start;

            let vertex_used = (vertex_count + wgpu::COPY_BUFFER_ALIGNMENT as usize)
                & !(wgpu::COPY_BUFFER_ALIGNMENT as usize - 1);
            let index_used = (index_count + wgpu::COPY_BUFFER_ALIGNMENT as usize)
                & !(wgpu::COPY_BUFFER_ALIGNMENT as usize - 1);

            let indices_new = &mut indices[span_start..index_cursor];
            if min_vindex != 0 {
                for v in indices_new.iter_mut() {
                    *v -= min_vindex as u32;
                }
            }
            let vertices_new = &vertices[(min_vindex as usize)..=(max_vindex as usize)];

            ctx.queue.write_buffer(
                &index_buffer,
                (*iused as u64) * std::mem::size_of::<u32>() as u64,
                any_as_u8_slice_array(indices_new),
            );
            ctx.queue.write_buffer(
                &vertex_buffer,
                (*vused as u64) * std::mem::size_of::<egui::paint::Vertex>() as u64,
                any_as_u8_slice_array(vertices_new),
            );
            self.stages.push(RenderStage {
                vertex_buffer: self.vertex_offset,
                index_buffer: self.index_offset,
                rect,
                base_index: *iused as u32,
                base_vertex: (*vused as u32) * std::mem::size_of::<egui::paint::Vertex>() as u32,
                count_idx: index_count as u32,
                texture_id,
            });
            *iused += index_used;
            *vused += vertex_used;
            if iremain < index_used + 3 {
                self.index_offset += 1;
            }
            if vremain < vertex_used + 4 {
                self.vertex_offset += 1;
            }
        }
        self.update_texture(ctx, &texture_bind_group_layout, ui_ctx, texture_id);
        // log::info!("render stages {:?}", self.stages);
    }

    fn prepare_vertex_buffer(&mut self, ctx: &mut RenderContext<'_>, vertex_offset: usize) {
        if vertex_offset >= self.vertex_buffers.len() {
            let buf = self.new_buffer(
                ctx,
                wgpu::BufferUsage::VERTEX,
                (DEFAULT_VERTEX_BUFFER_SIZE * std::mem::size_of::<egui::epaint::Vertex>()) as u64,
            );
            self.vertex_buffers.push((buf, 0));
        }
    }

    fn prepare_index_buffer(&mut self, ctx: &mut RenderContext<'_>, index_offset: usize) {
        if index_offset >= self.index_buffers.len() {
            let buf = self.new_buffer(
                ctx,
                wgpu::BufferUsage::INDEX,
                (DEFAULT_INDEX_BUFFER_SIZE * std::mem::size_of::<u32>()) as u64,
            );
            self.index_buffers.push((buf, 0));
        }
    }

    pub fn get_vertex_buffer(&self, idx: usize) -> &wgpu::Buffer {
        &self.vertex_buffers[idx].0
    }
    pub fn get_index_buffer(&self, idx: usize) -> &wgpu::Buffer {
        &self.index_buffers[idx].0
    }

    fn new_buffer(
        &self,
        ctx: &mut RenderContext<'_>,
        buffer_type: wgpu::BufferUsage,
        size: u64,
    ) -> wgpu::Buffer {
        let mat_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsage::COPY_DST | buffer_type,
            mapped_at_creation: false,
        });
        mat_buffer
    }

    fn update_texture(
        &mut self,
        ctx: &mut RenderContext<'_>,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        ui_ctx: egui::CtxRef,
        texture_id: egui::TextureId,
    ) {
        let (version, size, bytes) = match texture_id {
            egui::TextureId::Egui => {
                let t = ui_ctx.texture();
                (
                    t.version,
                    Size::new(t.width as u32, t.height as u32),
                    t.pixels.len(),
                )
            }
            egui::TextureId::User(_) => return,
        };
        let mut need_create = false;
        if let Some(t) = self.textures.get_mut(&texture_id) {
            if t.version == version {
                return;
            }
            t.version = version;
            if t.size != size {
                need_create = true;
                // destroy it
            }
        } else {
            need_create = true;
        }

        if need_create {
            let texture =
                self.new_texture(ctx, &texture_bind_group_layout, size, texture_id, version);
            self.textures.insert(texture_id, texture);
        }
        let texture = self.textures.get(&texture_id).unwrap();

        match texture_id {
            egui::TextureId::Egui => {
                self.copy_texture(ctx, texture, bytes, &ui_ctx.texture().pixels);
            }
            egui::TextureId::User(_) => {
                todo!("user bitmap");
            }
        };
    }

    fn copy_texture(
        &self,
        ctx: &mut RenderContext<'_>,
        texture: &Texture,
        bytes: usize,
        pixels: &[u8],
    ) {
        let size = texture.size;
        let dst = wgpu::ImageCopyTexture {
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            texture: &texture.texture,
        };
        let data_layout = wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: NonZeroU32::new((bytes / size.height as usize) as u32),
            rows_per_image: NonZeroU32::new(size.height as u32),
        };
        // copy texture data
        ctx.queue.write_texture(
            dst,
            pixels,
            data_layout,
            wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
        );
    }

    fn new_texture(
        &mut self,
        ctx: &mut RenderContext<'_>,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        size: Size,
        _id: egui::TextureId,
        version: u64,
    ) -> Texture {
        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
        });
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &texture_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            }],
        });
        Texture {
            texture,
            bind_group,
            size,
            version,
        }
    }
}
