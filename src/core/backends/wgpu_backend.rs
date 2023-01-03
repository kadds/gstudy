use std::{
    collections::VecDeque,
    num::{NonZeroU32, NonZeroU64},
    ops::{Not, Range},
    sync::{atomic::AtomicPtr, Arc, Mutex},
};

use crate::{
    core::context::{RContext, RContextImpl, RContextRef},
    event::{Event, EventProcessor, EventSource, ProcessEventResult},
    types::{Rectu, Size},
};
use anyhow::{anyhow, Result};
use dashmap::DashMap;
use wgpu::{
    util::{DeviceExt, StagingBelt},
    *,
};

#[derive(Debug)]
struct WGPUResourceInner {
    width: u32,
    height: u32,
}

#[derive(Debug)]
struct WGPUInstance {
    surface: Surface,
    inner: Mutex<WGPUResourceInner>,
    instance: Instance,
    adapter: Adapter,
    format: TextureFormat,
}

#[derive(Debug)]
pub struct WGPUResource {
    device: Device,
    queue: Queue,
    instance: Arc<WGPUInstance>,
    context: Arc<RContext>,
}

impl WGPUResource {
    fn build_surface_desc(width: u32, height: u32, format: TextureFormat) -> SurfaceConfiguration {
        SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Immediate,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
        }
    }
    pub fn device(&self) -> &Device {
        &self.device
    }
    pub fn queue(&self) -> &Queue {
        &self.queue
    }
    pub fn surface(&self) -> &Surface {
        &self.instance.surface
    }
    pub fn width(&self) -> u32 {
        let inner = self.instance.inner.lock().unwrap();
        inner.width
    }
    pub fn height(&self) -> u32 {
        let inner = self.instance.inner.lock().unwrap();
        inner.height
    }
    pub fn set_width_height(&self, width: u32, height: u32) {
        let mut inner = self.instance.inner.lock().unwrap();
        inner.width = width;
        inner.height = height;
    }
    pub fn format(&self) -> TextureFormat {
        self.instance.format
    }
    pub fn context(&self) -> &RContext {
        &self.context
    }
    pub fn context_ref(&self) -> RContextRef {
        self.context.clone()
    }

    pub fn new_queue(self: Arc<Self>) -> Arc<Self> {
        self
        // let device_fut = self.instance.adapter.request_device(
        //     &DeviceDescriptor {
        //         features: Features::empty(),
        //         limits: Limits::default(),
        //         label: Some("wgpu device"),
        //     },
        //     None,
        // );
        // #[cfg(not(target_arch = "wasm32"))]
        // let (device, queue) = pollster::block_on(device_fut).unwrap();

        // Arc::new(Self {
        //     instance: self.instance.clone(),
        //     device,
        //     queue,
        // })
    }
}

impl WGPUResource {
    pub fn copy_texture(
        &self,
        texture: &wgpu::Texture,
        bytes_per_pixel: u32,
        rectangle: Rectu,
        data: &[u8],
    ) {
        let dst = wgpu::ImageCopyTexture {
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: rectangle.x,
                y: rectangle.y,
                z: 0,
            },
            texture,
            aspect: wgpu::TextureAspect::All,
        };
        let row_bytes = rectangle.z * bytes_per_pixel;
        let data_layout = wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: NonZeroU32::new(row_bytes),
            rows_per_image: None,
        };

        self.queue().write_texture(
            dst,
            data,
            data_layout,
            wgpu::Extent3d {
                width: rectangle.z,
                height: rectangle.w,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn new_2d_texture(&self, label: Option<&'static str>, size: Size) -> wgpu::Texture {
        let device = self.device();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
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
        texture
    }

    pub fn new_srgba_2d_texture(&self, label: Option<&'static str>, size: Size) -> wgpu::Texture {
        let device = self.device();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        });
        texture
    }

    pub fn new_2d_attachment_texture(
        &self,
        label: Option<&'static str>,
        size: Size,
    ) -> wgpu::Texture {
        let device = self.device();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
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
        texture
    }

    pub fn new_depth_texture(&self, label: Option<&'static str>, size: Size) -> wgpu::Texture {
        let device = self.device();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
        });
        texture
    }
    pub fn new_sampler(&self, label: Option<&'static str>) -> wgpu::Sampler {
        self.device.create_sampler(&wgpu::SamplerDescriptor {
            label,
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
        })
    }

    pub fn new_wvp_buffer<T>(&self, label: Option<&'static str>) -> wgpu::Buffer {
        self.device.create_buffer(&BufferDescriptor {
            label,
            size: std::mem::size_of::<T>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
}

#[derive(Debug)]
pub struct WGPUBackend {
    inner: Arc<WGPUResource>,
}

pub struct WGPUEventProcessor {
    inner: Arc<WGPUResource>,
    format: TextureFormat,
}

#[derive(Debug)]
struct WGPUContextInner {}

#[derive(Debug)]
pub struct WGPUContext {
    inner: parking_lot::RwLock<WGPUContextInner>,
    pso_map: DashMap<u64, Arc<PipelinePass>>,
}

impl WGPUContext {
    pub fn new() -> Self {
        Self {
            inner: parking_lot::RwLock::new(WGPUContextInner {}),
            pso_map: DashMap::new(),
        }
    }
}

impl RContextImpl for WGPUContext {
    fn map_pso(&self, pso: u64, value: Option<PipelinePass>) {
        // let mut inner = self.inner.write();
        match value {
            Some(value) => {
                self.pso_map.insert(pso, Arc::new(value));
            }
            None => {
                self.pso_map.remove(&pso);
            }
        }
    }

    fn get_pso(&self, pso: u64) -> Arc<PipelinePass> {
        self.pso_map.get(&pso).unwrap().clone()
    }
}

impl WGPUBackend {
    pub fn new(window: &winit::window::Window) -> Result<WGPUBackend> {
        let bits = {
            #[cfg(not(target_arch = "wasm32"))]
            {
                wgpu::util::backend_bits_from_env().unwrap_or(wgpu::Backends::PRIMARY)
            }
            #[cfg(target_arch = "wasm32")]
            {
                wgpu::Backends::BROWSER_WEBGPU
            }
        };

        let instance = Instance::new(bits);
        let surface = unsafe { instance.create_surface(window) };
        let adapter_fut = instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        });
        let adapter_fut2 = instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        });
        let adapter_fut3 = instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
        });
        let adapter = {
            match pollster::block_on(adapter_fut) {
                Some(v) => v,
                None => {
                    // fallback to adapter config 2
                    match pollster::block_on(adapter_fut2) {
                        Some(v) => v,
                        None => pollster::block_on(adapter_fut3)
                            .ok_or_else(|| anyhow!("no adapter found"))?,
                    }
                }
            }
        };

        let device_fut = adapter.request_device(
            &DeviceDescriptor {
                features: Features::empty(),
                limits: Limits::default(),
                label: Some("wgpu device"),
            },
            None,
        );
        let device_fut2 = adapter.request_device(
            &DeviceDescriptor {
                features: Features::empty(),
                limits: Limits::downlevel_webgl2_defaults(),
                label: Some("wgpu device"),
            },
            None,
        );

        let (device, queue) = match pollster::block_on(device_fut) {
            Ok(v) => v,
            Err(e) => pollster::block_on(device_fut2)?,
        };

        let formats = surface.get_supported_formats(&adapter);
        let has_format = formats.iter().find(|v| **v == TextureFormat::Rgba8Unorm);
        let has_format_bgr = formats.iter().find(|v| **v == TextureFormat::Bgra8Unorm);
        let format = if has_format.is_some() {
            TextureFormat::Rgba8Unorm
        } else if has_format_bgr.is_some() {
            TextureFormat::Bgra8Unorm
        } else {
            anyhow::bail!("no texture format found")
        };
        log::info!("use format {:?}", format);

        Ok(WGPUBackend {
            inner: WGPUResource {
                context: RContext::new(Box::new(WGPUContext::new())),
                instance: Arc::new(WGPUInstance {
                    instance,
                    surface,
                    adapter,
                    inner: Mutex::new(WGPUResourceInner {
                        width: 0,
                        height: 0,
                    }),
                    format,
                }),
                device,
                queue,
            }
            .into(),
        })
    }
}

impl WGPUBackend {
    pub fn gpu(&self) -> Arc<WGPUResource> {
        self.inner.clone()
    }
}

#[derive(Debug)]
struct WGPUFrame {
    frame: SurfaceTexture,

    frame_texture_view: TextureView,
}

#[derive(Debug)]
pub struct WGPURenderer {
    inner: Arc<WGPUResource>,
    encoder: Option<CommandEncoder>,
    command_buffers: Vec<CommandBuffer>,
    first_call: bool,
}

struct WGPURenderTargetInner<'a, 'b> {
    color_attachments: Vec<RenderPassColorAttachment<'a>>,
    depth_attachment: Option<RenderPassDepthStencilAttachment<'a>>,
    render_pass_desc: RenderPassDescriptor<'a, 'b>,
}

impl<'a, 'b> WGPURenderTargetInner<'a, 'b> {
    pub fn new(label: &'static str) -> Self {
        Self {
            color_attachments: Vec::new(),
            render_pass_desc: RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[],
                depth_stencil_attachment: None,
            },
            depth_attachment: None,
        }
    }
    pub fn desc(&mut self) -> &RenderPassDescriptor<'a, 'b> {
        unsafe {
            self.render_pass_desc.color_attachments =
                std::mem::transmute(self.color_attachments.as_slice());
            self.render_pass_desc.depth_stencil_attachment =
                self.depth_attachment.clone().map(|v| v);
        }
        &self.render_pass_desc
    }
}

#[derive(Debug)]
pub struct WGPURenderTarget {
    inner: std::ptr::NonNull<u8>,
    tail_inner: std::ptr::NonNull<u8>,
    offset: u32,
}

unsafe impl core::marker::Send for WGPURenderTarget {}

impl WGPURenderTarget {
    pub fn new(label: &'static str) -> Self {
        let inner = Box::new(WGPURenderTargetInner::new(label));
        let ptr = Box::into_raw(inner);
        let tail_inner = Box::new(WGPURenderTargetInner::new(label));
        let tail_ptr = Box::into_raw(tail_inner);
        Self {
            inner: std::ptr::NonNull::new(ptr as *mut u8).unwrap(),
            tail_inner: std::ptr::NonNull::new(tail_ptr as *mut u8).unwrap(),
            offset: 0,
        }
    }
    fn get_mut<'a, 'b>(&mut self) -> &mut WGPURenderTargetInner<'a, 'b> {
        unsafe { std::mem::transmute(self.inner.as_ptr()) }
    }
    fn get_tail_mut<'a, 'b>(&mut self) -> &mut WGPURenderTargetInner<'a, 'b> {
        unsafe { std::mem::transmute(self.tail_inner.as_ptr()) }
    }
    pub fn reset(&mut self) {
        self.offset = 0;
    }

    pub fn desc<'a, 'b>(&mut self) -> &RenderPassDescriptor<'a, 'b> {
        self.offset += 1;
        if self.offset == 1 {
            unsafe { self.get_mut().desc() }
        } else {
            unsafe { self.get_tail_mut().desc() }
        }
    }

    fn map_ops(color: Option<crate::types::Color>) -> Operations<Color> {
        Operations {
            load: match color {
                Some(v) => LoadOp::Clear(wgpu::Color {
                    r: v.x as f64,
                    g: v.y as f64,
                    b: v.z as f64,
                    a: v.w as f64,
                }),
                None => LoadOp::Load,
            },
            store: true,
        }
    }

    pub fn set_depth_target(&mut self, view: &TextureView, clear: Option<f32>) {
        let inner = self.get_mut();
        let ops = Operations {
            load: match clear {
                Some(v) => LoadOp::Clear(v),
                None => LoadOp::Load,
            },
            store: true,
        };
        inner.depth_attachment = Some(RenderPassDepthStencilAttachment {
            view,
            depth_ops: Some(ops),
            stencil_ops: None,
        });
        let tail_inner = self.get_tail_mut();
        tail_inner.depth_attachment = Some(RenderPassDepthStencilAttachment {
            view,
            depth_ops: Some(Operations {
                load: LoadOp::Load,
                store: true,
            }),
            stencil_ops: None,
        })
    }

    pub fn set_render_target(
        &mut self,
        texture_view: &TextureView,
        color: Option<crate::types::Color>,
    ) {
        let inner = self.get_mut();
        let ops = Self::map_ops(color);
        if inner.color_attachments.len() == 0 {
            inner.color_attachments.push(RenderPassColorAttachment {
                view: texture_view,
                resolve_target: None,
                ops,
            })
        } else {
            inner.color_attachments[0] = RenderPassColorAttachment {
                view: texture_view,
                resolve_target: None,
                ops,
            }
        }
        let default_ops = Operations {
            load: LoadOp::Load,
            store: true,
        };
        let tail_inner = self.get_tail_mut();
        if tail_inner.color_attachments.len() == 0 {
            tail_inner
                .color_attachments
                .push(RenderPassColorAttachment {
                    view: texture_view,
                    resolve_target: None,
                    ops: default_ops,
                })
        } else {
            tail_inner.color_attachments[0] = RenderPassColorAttachment {
                view: texture_view,
                resolve_target: None,
                ops: default_ops,
            }
        }
    }
}

impl Drop for WGPURenderTarget {
    fn drop(&mut self) {}
}

#[derive(Debug)]
pub struct PassEncoder<'a> {
    renderer: &'a mut WGPURenderer,
    render_target: &'a mut WGPURenderTarget,
}

impl<'a> PassEncoder<'a> {
    pub fn new_pass<'b>(&'b mut self) -> RenderPass<'b> {
        let encoder = self.renderer.encoder.as_mut().unwrap();
        encoder.begin_render_pass(self.render_target.desc())
    }
    pub fn encoder(&self) -> &wgpu::CommandEncoder {
        self.renderer.encoder()
    }
    pub fn encoder_mut(&mut self) -> &mut wgpu::CommandEncoder {
        self.renderer.encoder_mut()
    }
}

impl WGPURenderer {
    pub fn new(gpu: Arc<WGPUResource>) -> Self {
        let encoder = gpu
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("wgpu encoder"),
            });
        Self {
            inner: gpu,
            encoder: Some(encoder),
            command_buffers: Vec::new(),
            first_call: true,
        }
    }
    pub fn resource(&self) -> Arc<WGPUResource> {
        self.inner.clone()
    }
    pub fn remake_encoder(&mut self) {
        self.command_buffers
            .push(self.encoder.take().unwrap().finish());
        let encoder = self
            .inner
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("wgpu encoder"),
            });
        self.encoder = Some(encoder);
    }

    pub fn encoder(&self) -> &wgpu::CommandEncoder {
        self.encoder.as_ref().unwrap()
    }
    pub fn encoder_mut(&mut self) -> &mut wgpu::CommandEncoder {
        self.encoder.as_mut().unwrap()
    }
}

impl WGPURenderer {
    pub fn begin<'a>(
        &'a mut self,
        render_target: &'a mut WGPURenderTarget,
    ) -> Option<PassEncoder<'a>> {
        if self.first_call {
            render_target.reset();
            self.first_call = false;
        }

        Some(PassEncoder {
            renderer: self,
            render_target,
        })
    }
}

impl Drop for WGPURenderer {
    fn drop(&mut self) {
        self.command_buffers
            .push(self.encoder.take().unwrap().finish());

        let mut tmp = Vec::new();

        std::mem::swap(&mut tmp, &mut self.command_buffers);

        self.inner.queue.submit(tmp.into_iter());
    }
}

impl WGPUBackend {
    pub fn event_processor(&self) -> Box<dyn EventProcessor> {
        Box::new(WGPUEventProcessor {
            inner: self.inner.clone(),
            format: self.inner.instance.format,
        })
    }
}

impl Renderer for WGPUBackend {
    fn renderer(&self) -> WGPURenderer {
        WGPURenderer::new(self.inner.clone())
    }
}

pub trait Renderer {
    fn renderer(&self) -> WGPURenderer;
}

impl EventProcessor for WGPUEventProcessor {
    fn on_event(&mut self, source: &dyn EventSource, event: &Event) -> ProcessEventResult {
        match event {
            Event::Resized(size) => {
                let width = u32::max(size.x, 16);
                let height = u32::max(size.y, 16);
                let format = self.format;

                self.inner.surface().configure(
                    &self.inner.device,
                    &WGPUResource::build_surface_desc(width, height, format),
                );
                self.inner.set_width_height(width, height);
                let _ = source.event_proxy().send_event(Event::JustRenderOnce);
            }
            Event::Render => {}
            _ => (),
        };
        ProcessEventResult::Received
    }
}

use std::collections::{BTreeMap, HashMap};

use spirq::{EntryPoint, Locator, ReflectConfig, SpirvBinary, Variable};

#[allow(dead_code)]
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum ShaderType {
    Vertex,
    Fragment,
    Compute,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub set: u32,
    pub binding: u32,
}

impl Position {
    pub fn new(set: u32, binding: u32) -> Self {
        Self { set, binding }
    }
}

#[derive(Debug)]
pub struct PipelinePass {
    pub pipeline: RenderPipeline,
    pub bind_group_layouts: Vec<BindGroupLayout>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct PipelineReflector<'a> {
    device: &'a Device,
    label: Option<&'static str>,
    vs: Option<ShaderModule>,
    fs: Option<(ShaderModule, FsTarget)>,
    cs: Option<ShaderModule>,
    vertex_attrs: BTreeMap<Position, VertexFormat>,
    bind_group_layout_entries: BTreeMap<Position, BindGroupLayoutEntry>,
    depth: Option<DepthStencilState>,
    err: Option<anyhow::Error>,
}

fn make_reflection(shader: &ShaderModuleDescriptor) -> SpirvBinary {
    match &shader.source {
        ShaderSource::SpirV(val) => val.as_ref().into(),
        _ => {
            panic!("un support shader binary");
        }
    }
}

#[derive(Debug)]
pub struct FsTarget {
    states: Vec<Option<ColorTargetState>>,
}

impl FsTarget {
    pub fn new_single(state: ColorTargetState) -> Self {
        Self {
            states: vec![Some(state)],
        }
    }

    pub fn new(fmt: TextureFormat) -> Self {
        let state = ColorTargetState {
            format: fmt,
            blend: None,
            write_mask: ColorWrites::all(),
        };
        Self::new_single(state)
    }

    pub fn new_blend_alpha_add_mix(fmt: TextureFormat) -> Self {
        let state = ColorTargetState {
            format: fmt,
            blend: Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::OneMinusDstAlpha,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
            }),
            write_mask: ColorWrites::all(),
        };
        Self::new_single(state)
    }
}

use lazy_static::lazy_static;
use spirq::ty::ScalarType;

use crate::core::ps::PrimitiveStateDescriptor;

lazy_static! {
    static ref SIGNED_MAP: HashMap<(u32, u32), VertexFormat> = {
        let mut map = HashMap::new();
        map.insert((1, 2), VertexFormat::Sint8x2);
        map.insert((1, 4), VertexFormat::Sint8x4);
        map.insert((2, 2), VertexFormat::Sint16x2);
        map.insert((2, 4), VertexFormat::Sint16x4);
        map.insert((4, 1), VertexFormat::Sint32);
        map.insert((4, 2), VertexFormat::Sint32x2);
        map.insert((4, 3), VertexFormat::Sint32x3);
        map.insert((4, 4), VertexFormat::Sint32x4);
        map
    };
    static ref UNSIGNED_MAP: HashMap<(u32, u32), VertexFormat> = {
        let mut map = HashMap::new();
        map.insert((1, 2), VertexFormat::Uint8x2);
        map.insert((1, 4), VertexFormat::Uint8x4);
        map.insert((2, 2), VertexFormat::Uint16x2);
        map.insert((2, 4), VertexFormat::Uint16x4);
        map.insert((4, 1), VertexFormat::Uint32);
        map.insert((4, 2), VertexFormat::Uint32x2);
        map.insert((4, 3), VertexFormat::Uint32x3);
        map.insert((4, 4), VertexFormat::Uint32x4);
        map
    };
    static ref FLOAT_MAP: HashMap<(u32, u32), VertexFormat> = {
        let mut map = HashMap::new();
        map.insert((2, 2), VertexFormat::Float16x2);
        map.insert((2, 4), VertexFormat::Float16x4);
        map.insert((4, 1), VertexFormat::Float32);
        map.insert((4, 2), VertexFormat::Float32x2);
        map.insert((4, 3), VertexFormat::Float32x3);
        map.insert((4, 4), VertexFormat::Float32x4);
        map.insert((8, 1), VertexFormat::Float64);
        map.insert((8, 2), VertexFormat::Float64x2);
        map.insert((8, 3), VertexFormat::Float64x3);
        map.insert((8, 4), VertexFormat::Float64x4);
        map
    };
}

fn scalar_to_wgpu_format(stype: &ScalarType, num: u32) -> Option<VertexFormat> {
    match stype {
        ScalarType::Signed(bits) => SIGNED_MAP.get(&(*bits, num)).copied(),
        ScalarType::Unsigned(bits) => UNSIGNED_MAP.get(&(*bits, num)).copied(),
        ScalarType::Float(bits) => FLOAT_MAP.get(&(*bits, num)).copied(),
        _ => None,
    }
}

fn image_to_wgpu_dimension(dim: spirv_headers::Dim, is_array: bool) -> TextureViewDimension {
    match dim {
        spirv_headers::Dim::Dim1D => {
            if is_array {
                todo!();
            }
            TextureViewDimension::D1
        }
        spirv_headers::Dim::Dim2D => {
            if is_array {
                TextureViewDimension::D2Array
            } else {
                TextureViewDimension::D2
            }
        }
        spirv_headers::Dim::Dim3D => {
            if is_array {
                todo!();
            }
            TextureViewDimension::D3
        }
        spirv_headers::Dim::DimCube => {
            if is_array {
                TextureViewDimension::CubeArray
            } else {
                TextureViewDimension::Cube
            }
        }
        spirv_headers::Dim::DimRect => {
            todo!();
        }
        spirv_headers::Dim::DimBuffer => todo!(),
        spirv_headers::Dim::DimSubpassData => todo!(),
    }
}

impl<'a> PipelineReflector<'a> {
    pub fn new(label: Option<&'static str>, device: &'a Device) -> Self {
        Self {
            label,
            device,
            vs: None,
            fs: None,
            cs: None,
            vertex_attrs: BTreeMap::new(),
            bind_group_layout_entries: BTreeMap::new(),
            depth: None,
            err: None,
        }
    }

    fn build_vertex_input(&mut self, entry: &EntryPoint) {
        for input in &entry.vars {
            let format = match input.ty() {
                spirq::ty::Type::Scalar(s) => scalar_to_wgpu_format(s, 1),
                spirq::ty::Type::Vector(t) => scalar_to_wgpu_format(&t.scalar_ty, t.nscalar),
                _ => {
                    continue;
                }
            }
            .unwrap();
            let locator = input.locator();
            if let Locator::Input(t) = locator {
                let position = Position::new(t.comp(), t.loc());
                self.vertex_attrs.insert(position, format);
            }
        }
    }

    fn build_bind_group_layout(
        &mut self,
        entry: &EntryPoint,
        ty: ShaderType,
    ) -> anyhow::Result<()> {
        let visibility = match ty {
            ShaderType::Vertex => ShaderStages::VERTEX,
            ShaderType::Fragment => ShaderStages::FRAGMENT,
            ShaderType::Compute => ShaderStages::COMPUTE,
        };

        for desc in &entry.vars {
            if let Variable::Descriptor {
                name,
                desc_bind,
                desc_ty,
                ty,
                nbind,
            } = desc
            {
                let binding = desc_bind.bind();
                let set = desc_bind.set();
                let entry = match desc_ty {
                    spirq::DescriptorType::UniformBuffer() => match ty {
                        spirq::ty::Type::Struct(struct_type) => Some(BindGroupLayoutEntry {
                            binding,
                            visibility: visibility,
                            count: None,
                            ty: BindingType::Buffer {
                                ty: BufferBindingType::Uniform,
                                has_dynamic_offset: true,
                                min_binding_size: None,
                            },
                        }),
                        _ => None,
                    },
                    spirq::DescriptorType::SampledImage() => match ty {
                        spirq::ty::Type::Image(img) => {
                            let multisampled = img.is_multisampled;

                            let sample_type = loop {
                                if let Some(is_depth) = img.is_depth {
                                    if is_depth {
                                        break TextureSampleType::Depth;
                                    }
                                }
                                if let Some(is_sampled) = img.is_sampled {
                                    if is_sampled {
                                        break TextureSampleType::Float { filterable: false };
                                    }
                                }
                                break TextureSampleType::Float { filterable: false };
                            };
                            let view_dimension = image_to_wgpu_dimension(img.dim, img.is_array);
                            Some(BindGroupLayoutEntry {
                                binding,
                                visibility,
                                count: None,
                                ty: BindingType::Texture {
                                    multisampled,
                                    view_dimension,
                                    sample_type,
                                },
                            })
                        }
                        spirq::ty::Type::SampledImage(sample) => {
                            let view_dimension =
                                image_to_wgpu_dimension(sample.dim, sample.is_array);
                            let multisampled = sample.is_multisampled;
                            let sample_type = TextureSampleType::Float { filterable: false };

                            Some(BindGroupLayoutEntry {
                                binding,
                                visibility,
                                count: None,
                                ty: BindingType::Texture {
                                    multisampled,
                                    view_dimension,
                                    sample_type,
                                },
                            })
                        }
                        _ => {
                            todo!()
                        }
                    },
                    spirq::DescriptorType::Sampler() => Some(BindGroupLayoutEntry {
                        binding,
                        visibility: visibility,
                        count: None,
                        ty: BindingType::Sampler(SamplerBindingType::NonFiltering),
                    }),
                    spirq::DescriptorType::InputAttachment(_) => todo!(),
                    spirq::DescriptorType::AccelStruct() => todo!(),
                    dt => {
                        log::error!("{:?}", dt);
                        None
                    }
                }
                .unwrap();
                let position = Position::new(set, binding);
                if let Some(item) = self.bind_group_layout_entries.get_mut(&position) {
                    if item.visibility != entry.visibility {
                        if item.ty != entry.ty {
                            anyhow::bail!("repeated binding in vs&fs at {}", entry.binding);
                        }
                    }
                } else {
                    self.bind_group_layout_entries.insert(position, entry);
                }
            }
        }
        Ok(())
    }

    pub fn add_vs(mut self, vs: ShaderModuleDescriptor) -> Self {
        let vs_ref = make_reflection(&vs);
        self.vs = Some(self.device.create_shader_module(vs));
        let entry = ReflectConfig::new()
            .spv(vs_ref)
            .ref_all_rscs(false)
            .reflect()
            .unwrap();
        self.build_vertex_input(&entry[0]);

        if let Err(err) = self.build_bind_group_layout(&entry[0], ShaderType::Vertex) {
            self.err = Some(err);
        }
        self
    }

    pub fn add_fs(mut self, fs: ShaderModuleDescriptor, fs_target: FsTarget) -> Self {
        let fs_ref = make_reflection(&fs);
        self.fs = Some((self.device.create_shader_module(fs), fs_target));
        let entry = ReflectConfig::new()
            .spv(fs_ref)
            .ref_all_rscs(false)
            .reflect()
            .unwrap();
        if let Err(err) = self.build_bind_group_layout(&entry[0], ShaderType::Fragment) {
            self.err = Some(err);
        }
        self
    }

    pub fn with_depth(mut self, depth: DepthStencilState) -> Self {
        self.depth = Some(depth);
        self
    }

    pub fn build(self, primitive: PrimitiveStateDescriptor) -> anyhow::Result<PipelinePass> {
        if let Some(err) = self.err {
            return Err(err);
        }

        let label = self.label;
        // build vertex buffer layout firstly
        let mut vertex_buffer_layouts = Vec::new();
        let mut vertex_attrs = Vec::new();
        {
            let mut ranges_size = Vec::new();
            let mut current = (0, 0);
            let mut offset = 0;

            for (pos, format) in self.vertex_attrs {
                if current.0 != pos.set {
                    if current.1 < vertex_attrs.len() {
                        ranges_size.push((current.1..vertex_attrs.len(), offset));
                    }
                    offset = 0;
                    current = (pos.set, vertex_attrs.len());
                }
                vertex_attrs.push(VertexAttribute {
                    format,
                    offset,
                    shader_location: pos.binding,
                });
                offset += format.size();
            }
            if current.1 < vertex_attrs.len() {
                ranges_size.push((current.1..vertex_attrs.len(), offset));
            }
            for (range, size) in ranges_size {
                vertex_buffer_layouts.push(VertexBufferLayout {
                    array_stride: size as BufferAddress,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &vertex_attrs[range],
                });
            }
        }

        // build bind groups secondly
        let mut layouts = Vec::new();
        {
            let mut layout_entries = Vec::new();
            let mut current = Position::new(u32::MAX, u32::MAX);
            for (pos, entry) in self.bind_group_layout_entries {
                if current.set != pos.set {
                    if !layout_entries.is_empty() {
                        let layout =
                            self.device
                                .create_bind_group_layout(&BindGroupLayoutDescriptor {
                                    label,
                                    entries: &layout_entries,
                                });
                        layouts.push(layout);
                        layout_entries.clear();
                    }
                }
                current = pos;
                layout_entries.push(entry);
            }
            if !layout_entries.is_empty() {
                let layout = self
                    .device
                    .create_bind_group_layout(&BindGroupLayoutDescriptor {
                        label,
                        entries: &layout_entries,
                    });
                layouts.push(layout);
            }
        }
        let mut ref_layouts = Vec::new();
        for layout in &layouts {
            ref_layouts.push(layout);
        }
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label,
                bind_group_layouts: &ref_layouts,
                push_constant_ranges: &[],
            });

        if self.depth.is_some() {
            log::info!("{:?} init with depth", label);
        }
        let primitive = primitive.into();

        let mut pipeline_desc = RenderPipelineDescriptor {
            label,
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &self.vs.unwrap(),
                entry_point: "main",
                buffers: &vertex_buffer_layouts,
            },
            fragment: None,
            primitive,
            depth_stencil: self.depth,
            multisample: MultisampleState::default(),
            multiview: None,
        };

        if let Some(fs) = &self.fs {
            pipeline_desc.fragment = Some(FragmentState {
                module: &fs.0,
                entry_point: "main",
                targets: &fs.1.states,
            })
        }

        log::info!("{:?}", pipeline_desc);

        let pipeline = self.device.create_render_pipeline(&pipeline_desc);

        Ok(PipelinePass {
            pipeline,
            bind_group_layouts: layouts,
        })
    }

    pub fn depth_with_format(format: TextureFormat) -> DepthStencilState {
        let stencil = StencilState {
            front: StencilFaceState::IGNORE,
            back: StencilFaceState::IGNORE,
            write_mask: 0,
            read_mask: 0,
        };
        DepthStencilState {
            format,
            depth_write_enabled: true,
            depth_compare: CompareFunction::LessEqual,
            stencil,
            bias: DepthBiasState {
                constant: 0,
                slope_scale: 0.0f32,
                clamp: 0.0,
            },
        }
    }
}

impl From<crate::core::ps::Topology> for PrimitiveTopology {
    fn from(value: crate::core::ps::Topology) -> Self {
        match value {
            crate::core::ps::Topology::PointList => Self::PointList,
            crate::core::ps::Topology::LineList => Self::LineList,
            crate::core::ps::Topology::LineStrip => Self::LineStrip,
            crate::core::ps::Topology::TriangleList => Self::TriangleList,
            crate::core::ps::Topology::TriangleStrip => Self::TriangleStrip,
        }
    }
}

impl From<crate::core::ps::CullFace> for Option<Face> {
    fn from(value: crate::core::ps::CullFace) -> Self {
        match value {
            crate::core::ps::CullFace::None => None,
            crate::core::ps::CullFace::Front => Some(Face::Front),
            crate::core::ps::CullFace::Back => Some(Face::Back),
        }
    }
}

impl From<crate::core::ps::PolygonMode> for PolygonMode {
    fn from(value: crate::core::ps::PolygonMode) -> Self {
        match value {
            crate::core::ps::PolygonMode::Fill => Self::Fill,
            crate::core::ps::PolygonMode::Line => Self::Line,
            crate::core::ps::PolygonMode::Point => Self::Point,
        }
    }
}

impl From<PrimitiveStateDescriptor> for PrimitiveState {
    fn from(p: PrimitiveStateDescriptor) -> Self {
        Self {
            topology: p.topology().into(),
            strip_index_format: None,
            front_face: FrontFace::Ccw,
            cull_mode: p.cull_face().into(),
            unclipped_depth: false,
            polygon_mode: p.polygon().into(),
            conservative: false,
        }
    }
}

pub struct SharedBuffers {
    buffers: Vec<wgpu::Buffer>,
    buf_size: u32,
}

impl SharedBuffers {
    pub fn new(max_size: u32) -> Self {
        Self {
            buffers: Vec::new(),
            buf_size: max_size,
        }
    }

    fn new_buffer(&mut self, gpu: &WGPUResource) {
        // self.buffers.push(gpu.device.create_buffer())
    }

    pub fn new_frame(&mut self) {}

    pub fn commit(&mut self) {}
}
pub struct GpuMainBuffer {
    buffer: wgpu::Buffer,
    size: u64,
    label: Option<&'static str>,
    usage: wgpu::BufferUsages,

    recent_usage_size: VecDeque<u64>,
    tick: u64,
}

impl GpuMainBuffer {
    pub fn new(gpu: &WGPUResource, label: Option<&'static str>, usage: wgpu::BufferUsages) -> Self {
        let size = 1024 * 1024 * 2; // 2mb

        let buffer = gpu.device().create_buffer(&wgpu::BufferDescriptor {
            label: label,
            size: size,
            usage: wgpu::BufferUsages::COPY_DST | usage,
            mapped_at_creation: false,
        });
        let mut recent_usage_size = VecDeque::new();
        recent_usage_size.push_back(size);

        Self {
            label,
            size,
            buffer,
            usage,
            recent_usage_size,
            tick: 0,
        }
    }

    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    pub fn new_vertex(gpu: &WGPUResource, label: Option<&'static str>) -> Self {
        Self::new(gpu, label, wgpu::BufferUsages::VERTEX)
    }

    pub fn new_index(gpu: &WGPUResource, label: Option<&'static str>) -> Self {
        Self::new(gpu, label, wgpu::BufferUsages::INDEX)
    }

    pub fn make_sure(&mut self, mut size: u64, gpu: &WGPUResource) -> bool {
        self.tick += 1;
        for _ in 0..1 {
            if self.size < size {
                self.size = size;

                self.buffer = gpu.device().create_buffer(&wgpu::BufferDescriptor {
                    label: self.label,
                    size: self.size,
                    usage: wgpu::BufferUsages::COPY_DST | self.usage,
                    mapped_at_creation: false,
                });
                self.recent_usage_size.truncate(0);
                self.recent_usage_size.push_back(size);
                return true;
            } else {
                if self.recent_usage_size.len() > 100 {
                    self.recent_usage_size.pop_front().unwrap();
                }
                self.recent_usage_size.push_back(size);

                if self.tick % 500 == 0 {
                    let max_val = self.recent_usage_size.iter().max().unwrap();
                    if *max_val < self.size / 2 {
                        size = self.size / 2;
                        continue;
                    }
                }
            }
            return false;
        }
        return true;
    }
}

struct GpuInputMainBuffer {
    buffer: GpuMainBuffer,
    stage: StagingBelt,
    chunk_size: u64,
    alignment: u64,
    offset: u64,
}

impl GpuInputMainBuffer {
    pub fn new(gpu: &WGPUResource, label: Option<&'static str>, usage: wgpu::BufferUsages) -> Self {
        let chunk_size = 1024 * 1024 * 4;

        Self {
            buffer: GpuMainBuffer::new(gpu, label, usage),
            stage: StagingBelt::new(chunk_size),
            chunk_size,
            offset: 0,
            alignment: wgpu::COPY_BUFFER_ALIGNMENT,
        }
    }

    pub fn set_alignment(&mut self, a: u64) {
        self.alignment = a;
    }

    #[inline]
    pub fn recall(&mut self) {
        self.stage.recall();
        self.offset = 0;
    }

    #[inline]
    pub fn finish(&mut self) {
        self.stage.finish();
    }

    #[inline]
    pub fn make_sure(&mut self, gpu: &WGPUResource, bytes: u64) -> bool {
        self.buffer.make_sure(bytes + self.offset, gpu)
    }

    pub fn copy_stage(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        gpu: &WGPUResource,
        data: &[u8],
    ) -> Range<u64> {
        let size = data.len() as u64;

        let mut bytes = self.stage.write_buffer(
            encoder,
            self.buffer.buffer(),
            self.offset,
            NonZeroU64::new(size).unwrap(),
            gpu.device(),
        );
        bytes.copy_from_slice(data);

        let range = self.offset..(self.offset + size);
        self.offset = (range.end + self.alignment - 1) & (self.alignment - 1).not();

        range
    }

    pub fn buffer(&self) -> &wgpu::Buffer {
        self.buffer.buffer()
    }
}

pub struct GpuInputUniformBuffers {
    buffers: Vec<wgpu::Buffer>,
    label: Option<&'static str>,
    stage: StagingBelt,

    recent_usage_size: VecDeque<u64>,
    tick: u64,

    index: u64,
    offset: u64,
    size: u64,
    alignment: u64,
}

impl GpuInputUniformBuffers {
    pub fn new(gpu: &WGPUResource, label: Option<&'static str>) -> Self {
        let size = gpu.device.limits().max_uniform_buffer_binding_size;
        let alignment = gpu.device.limits().min_uniform_buffer_offset_alignment;

        Self {
            buffers: Vec::new(),
            label,
            stage: StagingBelt::new(size as u64),

            recent_usage_size: VecDeque::new(),
            tick: 0,
            index: 0,
            offset: 0,

            size: size as u64,
            alignment: alignment as u64,
        }
    }

    #[inline]
    pub fn recall(&mut self) {
        self.stage.recall();
        self.index = 0;
        self.offset = 0;
    }

    #[inline]
    pub fn finish(&mut self) {
        self.stage.finish();
    }

    #[inline]
    pub fn make_sure(&mut self, gpu: &WGPUResource, n: u64, mut single_bytes: u64) -> bool {
        single_bytes = (single_bytes + self.alignment - 1) & (self.alignment - 1).not();

        let elements_for_uniform_buffer = self.size / single_bytes;
        assert!(elements_for_uniform_buffer != 0);
        let total_buffers =
            (n + elements_for_uniform_buffer + 1) / elements_for_uniform_buffer + self.index;

        let mut any_changes = false;

        while self.buffers.len() < total_buffers as usize {
            let buffer = gpu.device().create_buffer(&wgpu::BufferDescriptor {
                label: self.label,
                size: self.size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
                mapped_at_creation: false,
            });
            self.buffers.push(buffer);
            any_changes = true;
        }
        any_changes
    }

    pub fn copy_stage(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        gpu: &WGPUResource,
        data: &[u8],
    ) -> BufferPosition {
        let rest = self.size - self.offset;
        let size = data.len() as u64;

        if rest < size {
            self.index += 1;
            self.offset = 0;
        }

        let mut bytes = self.stage.write_buffer(
            encoder,
            &self.buffers[self.index as usize],
            self.offset,
            NonZeroU64::new(size).unwrap(),
            gpu.device(),
        );
        bytes.copy_from_slice(data);

        let range = self.offset..(self.offset + size);
        self.offset = (range.end + self.alignment - 1) & (self.alignment - 1).not();

        BufferPosition {
            index: self.index,
            range,
        }
    }

    pub fn buffer(&self, index: u64) -> &wgpu::Buffer {
        &self.buffers[index as usize]
    }

    pub fn buffers(&self) -> &[wgpu::Buffer] {
        &self.buffers
    }
}

pub struct GpuInputMainBuffers {
    index: GpuInputMainBuffer,
    vertex: GpuInputMainBuffer,
}

impl GpuInputMainBuffers {
    pub fn new(gpu: &WGPUResource, label: Option<&'static str>) -> Self {
        Self {
            index: GpuInputMainBuffer::new(gpu, label, wgpu::BufferUsages::INDEX),
            vertex: GpuInputMainBuffer::new(gpu, label, wgpu::BufferUsages::VERTEX),
        }
    }

    pub fn make_sure(&mut self, gpu: &WGPUResource, index_bytes: u64, vertex_bytes: u64) {
        self.index.make_sure(gpu, index_bytes);
        self.vertex.make_sure(gpu, vertex_bytes);
    }

    pub fn copy_stage(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        gpu: &WGPUResource,
        indices: &[u8],
        vertices: &[u8],
    ) -> (Range<u64>, Range<u64>) {
        let index = self.index.copy_stage(encoder, gpu, indices);
        let vertex = self.vertex.copy_stage(encoder, gpu, vertices);
        (index, vertex)
    }

    pub fn recall(&mut self) {
        self.index.recall();
        self.vertex.recall();
    }

    pub fn finish(&mut self) {
        self.index.finish();
        self.vertex.finish();
    }

    pub fn vertex_buffer(&self) -> &wgpu::Buffer {
        self.vertex.buffer()
    }
    pub fn vertex_buffer_slice(&self, range: Range<u64>) -> wgpu::BufferSlice {
        self.vertex.buffer().slice(range)
    }
    pub fn index_buffer(&self) -> &wgpu::Buffer {
        self.index.buffer()
    }
    pub fn index_buffer_slice(&self, range: Range<u64>) -> wgpu::BufferSlice {
        self.index.buffer().slice(range)
    }
}

pub struct GpuInputMainBuffersWithUniform {
    index: GpuInputMainBuffer,
    vertex: GpuInputMainBuffer,
    uniform: GpuInputUniformBuffers,
}

pub struct BufferPosition {
    pub index: u64,
    pub range: Range<u64>,
}

impl GpuInputMainBuffersWithUniform {
    pub fn new(gpu: &WGPUResource, label: Option<&'static str>) -> Self {
        let mut uniform = GpuInputUniformBuffers::new(gpu, label);

        Self {
            index: GpuInputMainBuffer::new(gpu, label, wgpu::BufferUsages::INDEX),
            vertex: GpuInputMainBuffer::new(gpu, label, wgpu::BufferUsages::VERTEX),
            uniform,
        }
    }

    pub fn make_sure(
        &mut self,
        gpu: &WGPUResource,
        index_bytes: u64,
        vertex_bytes: u64,
        n_uniform: u64,
        single_bytes: u64,
    ) -> bool {
        self.index.make_sure(gpu, index_bytes);
        self.vertex.make_sure(gpu, vertex_bytes);
        let changed = self.uniform.make_sure(gpu, n_uniform, single_bytes);
        changed
    }

    pub fn copy_stage(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        gpu: &WGPUResource,
        indices: &[u8],
        vertices: &[u8],
        uniforms: &[u8],
    ) -> (Range<u64>, Range<u64>, BufferPosition) {
        let index = self.index.copy_stage(encoder, gpu, indices);
        let vertex = self.vertex.copy_stage(encoder, gpu, vertices);
        let uniform = self.uniform.copy_stage(encoder, gpu, uniforms);
        (index, vertex, uniform)
    }

    pub fn recall(&mut self) {
        self.index.recall();
        self.vertex.recall();
        self.uniform.recall();
    }

    pub fn finish(&mut self) {
        self.index.finish();
        self.vertex.finish();
        self.uniform.finish();
    }

    pub fn vertex_buffer(&self) -> &wgpu::Buffer {
        self.vertex.buffer()
    }
    pub fn vertex_buffer_slice(&self, range: Range<u64>) -> wgpu::BufferSlice {
        self.vertex.buffer().slice(range)
    }
    pub fn index_buffer(&self) -> &wgpu::Buffer {
        self.index.buffer()
    }
    pub fn index_buffer_slice(&self, range: Range<u64>) -> wgpu::BufferSlice {
        self.index.buffer().slice(range)
    }

    pub fn uniform_buffer(&self, buffer_position: BufferPosition) -> &wgpu::Buffer {
        self.uniform.buffer(buffer_position.index)
    }
    pub fn uniform_buffers(&self) -> &[wgpu::Buffer] {
        self.uniform.buffers()
    }
}
