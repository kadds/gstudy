use crate::{
    gpu_context::{GpuContext, GpuContextRef, GpuInstance, GpuInstanceRef},
    modules::hardware_renderer::common::{FsTarget, PipelinePass, PipelineReflector, Position},
    render_window::{RenderWindowEvent, RenderWindowInputEvent, UserEvent, WindowUserEvent},
    statistics::Statistics,
    types::{self, Vec4f},
    util::*,
};
use std::{
    num::NonZeroU32,
    time::{Duration, Instant},
};

use egui::{CtxRef, RawInput};
use winit::{event::WindowEvent, event_loop::EventLoopProxy};

use self::logic::UILogicRef;

pub mod logic;

mod subwindow;

type Size = types::Size;
type Rect = types::Point4<u32>;

use wgpu::*;

#[derive(Debug)]
pub struct RenderContext<'a> {
    pub queue: &'a Queue,
    pub device: &'a Device,
    pub encoder: &'a mut CommandEncoder,
}

#[derive(Debug, Clone)]
struct MatBuffer {
    size: [f32; 2],
}

struct Inner {
    pipeline_pass: PipelinePass,
    bind_group: wgpu::BindGroup,
    mat_buffer: wgpu::Buffer,
    inner_size: Size,
    size_changed: bool,
    last_mouse_pos: (f32, f32),
    last_modifiers: egui::Modifiers,
    dyn_state: DynRenderState,
    meshes: Option<Vec<egui::ClippedMesh>>,
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

pub struct Texture {
    pub texture: wgpu::Texture,
    pub bind_group: wgpu::BindGroup,
    pub size: Size,
    pub version: u64,
}

struct DynRenderState {
    texture: Option<Texture>,
    stages: Vec<RenderStage>,
    vertex_buffers: Vec<(wgpu::Buffer, usize)>,
    vertex_offset: usize,
    index_buffers: Vec<(wgpu::Buffer, usize)>,
    index_offset: usize,
}

pub struct UIRenderer {
    ui_ctx: egui::CtxRef,
    input: RawInput,
    inner: Inner,
    clear_color: Option<wgpu::Color>,
    gpu_context: GpuContextRef,
    gpu_instance: GpuInstanceRef,
    ui_logic: UILogicRef,
    event_proxy: EventLoopProxy<UserEvent>,
    cursor: egui::CursorIcon,
}

impl UIRenderer {
    pub fn new(
        gpu_context: GpuContextRef,
        size: Size,
        event_proxy: EventLoopProxy<UserEvent>,
        ui_logic: UILogicRef,
    ) -> Self {
        let instance = gpu_context.instance();
        log::info!("new ui renderer {:?}", instance.id());
        let inner = Self::init_renderer(&instance, size);
        Self {
            ui_ctx: egui::CtxRef::default(),
            input: RawInput::default(),
            inner,
            clear_color: Some(wgpu::Color::BLACK),
            gpu_context: gpu_context.clone(),
            ui_logic,
            gpu_instance: instance,
            event_proxy,
            cursor: egui::CursorIcon::Default,
        }
    }

    pub fn logic(&self) -> UILogicRef {
        self.ui_logic.clone()
    }

    pub fn event_proxy(&self) -> EventLoopProxy<UserEvent> {
        self.event_proxy.clone()
    }

    pub fn set_clear_color(&mut self, c: Option<Vec4f>) {
        unsafe {
            let color = c.map(|c| wgpu::Color {
                r: c.get_unchecked(0).clone() as f64,
                g: c.get_unchecked(1).clone() as f64,
                b: c.get_unchecked(2).clone() as f64,
                a: c.get_unchecked(3).clone() as f64,
            });
            log::info!("clear color {:?}", color);
            self.clear_color = color;
        }
    }

    pub fn render(&mut self) {
        let frame = match self.gpu_instance.surface().get_current_texture() {
            Ok(v) => v,
            Err(e) => {
                log::error!("get swapchain fail. {}", e);
                return;
            }
        };

        let mut encoder =
            self.gpu_instance
                .device()
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("ui encoder"),
                });
        self.prepare_render(&mut encoder);
        {
            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let render_pass_desc = RenderPassDescriptor {
                label: None,
                color_attachments: &[RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: Operations {
                        load: self
                            .clear_color
                            .map_or_else(|| wgpu::LoadOp::Load, |v| wgpu::LoadOp::Clear(v)),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            };
            let mut render_pass = encoder.begin_render_pass(&render_pass_desc);
            self.render_inner(&mut render_pass);
        }
        self.gpu_instance
            .queue()
            .submit(std::iter::once(encoder.finish()));
        frame.present();
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.gpu_context.rebuild(Size::new(width, height));
    }

    pub fn rebind_window(&mut self, logic_window_id: u64) {
        self.ui_logic.rebind_logic_window(logic_window_id);
    }

    pub fn update(&mut self, statistics: &Statistics) -> bool {
        let inner = &mut self.inner;
        let mut input = RawInput::default();
        std::mem::swap(&mut input, &mut self.input);
        let time: Duration = statistics.elapsed();
        input.time = Some(time.as_micros() as f64 / 1000_000f64);
        input.modifiers = inner.last_modifiers.clone();

        let ui_ctx = &mut self.ui_ctx;

        ui_ctx.begin_frame(input);
        self.ui_logic
            .update(ui_ctx.clone(), statistics, &self.event_proxy);
        let (output, shapes) = ui_ctx.end_frame();
        let meshes = ui_ctx.tessellate(shapes);
        inner.meshes = Some(meshes);
        self.ui_logic
            .finish(&output, self.cursor, &self.event_proxy);
        output.needs_repaint
    }

    fn prepare_render(&mut self, encoder: &mut CommandEncoder) {
        let inner = &mut self.inner;
        let state = &mut inner.dyn_state;
        state.new_frame();

        if inner.size_changed {
            let mat_buffer = MatBuffer {
                size: [inner.inner_size.x as f32, inner.inner_size.y as f32],
            };
            self.gpu_instance.queue().write_buffer(
                &inner.mat_buffer,
                0,
                any_as_u8_slice(&mat_buffer),
            );
            inner.size_changed = false;
        }
        let meshes = inner.meshes.as_mut().unwrap();

        for mesh in meshes {
            let rect = mesh.0;
            let mesh = &mut mesh.1;
            let x = (rect.left() as u32).clamp(0, inner.inner_size.x);
            let y = (rect.top() as u32).clamp(0, inner.inner_size.y);
            let width = (rect.width() as u32).clamp(0, inner.inner_size.x - x);
            let height = (rect.height() as u32).clamp(0, inner.inner_size.y - y);

            if width == 0 || height == 0 {
                continue;
            }
            let rect = Rect::new(x, y, width, height);

            state.commit(
                &self.gpu_instance,
                &mut mesh.indices,
                &mut mesh.vertices,
                rect,
                mesh.texture_id,
                &inner.pipeline_pass.bind_group_layouts[1],
                self.ui_ctx.clone(),
            );
        }
        self.ui_logic.prepare_texture();
    }
    fn render_inner<'a>(&'a mut self, pass: &mut wgpu::RenderPass<'a>) {
        let inner = &mut self.inner;
        let state = &mut inner.dyn_state;
        pass.set_pipeline(&inner.pipeline_pass.pipeline);
        pass.set_bind_group(0, &inner.bind_group, &[]);
        for stage in &state.stages {
            let rect = stage.rect;
            pass.set_scissor_rect(rect.x as u32, rect.y as u32, rect.z as u32, rect.w as u32);
            let texture_bind_group = match stage.texture_id {
                egui::TextureId::Egui => &state.texture.as_ref().unwrap().bind_group,
                egui::TextureId::User(id) => self.ui_logic.get_texture(id),
            };

            // bind texture
            pass.set_bind_group(1, texture_bind_group, &[]);
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

    fn init_renderer(gpu: &GpuInstance, size: Size) -> Inner {
        let device = gpu.device();
        let pipeline_pass = PipelineReflector::new(Some("ui"), device)
            .add_vs(&wgpu::include_spirv!("../compile_shaders/ui.vert"))
            .add_fs(
                &wgpu::include_spirv!("../compile_shaders/ui.frag"),
                FsTarget::new_blend_alpha_add_mix(wgpu::TextureFormat::Rgba8UnormSrgb),
            )
            .build_default();

        let mat_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui"),
            size: 8,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ui"),
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
            label: Some("ui"),
            layout: &pipeline_pass.bind_group_layouts[0],
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
        log::info!("init renderer done");

        Inner {
            pipeline_pass,
            bind_group,
            mat_buffer,
            dyn_state: DynRenderState::new(),
            inner_size: size,
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
        }
    }

    pub fn on_event(&mut self, event: &RenderWindowEvent) {
        let inner = &mut self.inner;
        match event {
            RenderWindowEvent::Resized(size) => {
                inner.inner_size = *size;
                inner.size_changed = true;
                self.input.screen_rect = Some(egui::Rect {
                    min: egui::Pos2::new(0f32, 0f32),
                    max: egui::Pos2::new(size.x as f32, size.y as f32),
                });
                self.resize(size.x as u32, size.y as u32);
            }
            RenderWindowEvent::UserEvent(event) => match event {
                WindowUserEvent::UpdateCursor(cursor) => {
                    self.cursor = *cursor;
                }
                _ => (),
            },
            RenderWindowEvent::Input(event) => match event {
                RenderWindowInputEvent::ModifiersChanged(state) => {
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
                RenderWindowInputEvent::KeyboardInput {
                    device_id: _,
                    input,
                    is_synthetic: _,
                } => {
                    let key = match match_egui_key(
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
                RenderWindowInputEvent::MouseWheel {
                    device_id: _,
                    delta,
                    phase: _,
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
                RenderWindowInputEvent::CursorLeft { device_id: _ } => {
                    self.input.events.push(egui::Event::PointerGone);
                }
                &RenderWindowInputEvent::CursorMoved {
                    device_id: _,
                    position,
                } => {
                    self.input
                        .events
                        .push(egui::Event::PointerMoved(egui::Pos2::new(
                            position.x as f32,
                            position.y as f32,
                        )));
                    inner.last_mouse_pos = (position.x as f32, position.y as f32);
                }
                RenderWindowInputEvent::MouseInput {
                    device_id: _,
                    state,
                    button,
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
                RenderWindowInputEvent::ReceivedCharacter(c) => {
                    let c = *c;
                    if !c.is_ascii_control() {
                        self.input.events.push(egui::Event::Text(c.to_string()));
                    }
                }
                _ => (),
            },
            _ => (),
        };
    }
}

impl DynRenderState {
    pub fn new() -> Self {
        Self {
            texture: None,
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
        gpu: &GpuInstance,
        indices: &mut [u32],
        vertices: &mut [egui::epaint::Vertex],
        rect: Rect,
        texture_id: egui::TextureId,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        ui_ctx: CtxRef,
    ) {
        let mut index_cursor = 0;

        while index_cursor < indices.len() {
            self.prepare_index_buffer(gpu, self.index_offset);
            self.prepare_vertex_buffer(gpu, self.vertex_offset);

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

            let queue = gpu.queue();

            queue.write_buffer(
                &index_buffer,
                (*iused as u64) * std::mem::size_of::<u32>() as u64,
                any_as_u8_slice_array(indices_new),
            );
            queue.write_buffer(
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
        self.update_texture(gpu, &texture_bind_group_layout, ui_ctx);
        // log::info!("render stages {:?}", self.stages);
    }

    fn prepare_vertex_buffer(&mut self, gpu: &GpuInstance, vertex_offset: usize) {
        if vertex_offset >= self.vertex_buffers.len() {
            let buf = self.new_buffer(
                gpu,
                wgpu::BufferUsages::VERTEX,
                (DEFAULT_VERTEX_BUFFER_SIZE * std::mem::size_of::<egui::epaint::Vertex>()) as u64,
            );
            self.vertex_buffers.push((buf, 0));
        }
    }

    fn prepare_index_buffer(&mut self, gpu: &GpuInstance, index_offset: usize) {
        if index_offset >= self.index_buffers.len() {
            let buf = self.new_buffer(
                gpu,
                wgpu::BufferUsages::INDEX,
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
        gpu: &GpuInstance,
        buffer_type: wgpu::BufferUsages,
        size: u64,
    ) -> wgpu::Buffer {
        let mat_buffer = gpu.device().create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsages::COPY_DST | buffer_type,
            mapped_at_creation: false,
        });
        mat_buffer
    }

    fn update_texture(
        &mut self,
        gpu: &GpuInstance,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        ui_ctx: egui::CtxRef,
    ) {
        let (version, size, bytes) = {
            let t = ui_ctx.texture();
            (
                t.version,
                Size::new(t.width as u32, t.height as u32),
                t.pixels.len(),
            )
        };
        let mut need_create = false;
        if let Some(t) = self.texture.as_mut() {
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
            let texture = self.new_texture(gpu, &texture_bind_group_layout, size, version);
            self.texture = Some(texture);
        }
        let texture = self.texture.as_ref().unwrap();

        self.copy_texture(gpu, texture, bytes, &ui_ctx.texture().pixels);
    }

    fn copy_texture(
        &self,
        gpu: &GpuInstance,
        texture: &Texture,
        bytes: usize,
        fixed_pixels: &[u8],
    ) {
        let size = texture.size;
        let dst = wgpu::ImageCopyTexture {
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            texture: &texture.texture,
            aspect: wgpu::TextureAspect::All,
        };
        let data_layout = wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: NonZeroU32::new(((bytes / size.y as usize) * 4) as u32),
            rows_per_image: NonZeroU32::new(size.y as u32),
        };
        // copy texture data
        let mut pixels: Vec<u8> = Vec::with_capacity(fixed_pixels.len() * 4);
        for srgba in fixed_pixels {
            pixels.push(*srgba);
            pixels.push(*srgba);
            pixels.push(*srgba);
            pixels.push(*srgba);
        }
        gpu.queue().write_texture(
            dst,
            &pixels,
            data_layout,
            wgpu::Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
        );
    }

    fn new_texture(
        &mut self,
        gpu: &GpuInstance,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        size: Size,
        version: u64,
    ) -> Texture {
        let device = gpu.device();
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
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        });
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
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
