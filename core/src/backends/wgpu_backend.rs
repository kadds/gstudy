use std::{
    any::Any,
    collections::{HashMap, HashSet, VecDeque},
    num::NonZeroU64,
    ops::{Not, Range},
    sync::{Arc, Mutex},
};

use crate::{
    context::{RContext, RContextRef, ResourceRef},
    event::{Event, EventProcessor, EventSource, ProcessEventResult},
    render::common::BufferAccessor,
    types::{Rectu, Size, Vec4},
    util::any_as_u8_slice_array,
};
use anyhow::{anyhow, Result};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt, StagingBelt},
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
    #[allow(unused)]
    instance: Instance,
    #[allow(unused)]
    adapter: Adapter,
    format: TextureFormat,
}

pub struct WindowSurfaceFrame<'a> {
    texture: Option<ResourceRef>,
    s: Option<Arc<wgpu::SurfaceTexture>>,
    gpu: &'a WGPUResource,
}

impl<'a> WindowSurfaceFrame<'a> {
    pub fn texture(&self) -> ResourceRef {
        self.texture.as_ref().unwrap().clone()
    }
    pub fn surface_texture(&self) -> Arc<wgpu::SurfaceTexture> {
        self.s.as_ref().unwrap().clone()
    }
}

impl<'a> Drop for WindowSurfaceFrame<'a> {
    fn drop(&mut self) {
        let t = self.texture.take().unwrap();
        self.gpu.context.deregister(t);

        Arc::try_unwrap(self.s.take().unwrap()).unwrap().present();
    }
}

pub struct WindowSurfaceFrame2 {
    texture: Option<ResourceRef>,
    s: Option<Arc<wgpu::SurfaceTexture>>,
    gpu: Arc<WGPUResource>,
}

impl WindowSurfaceFrame2 {
    pub fn texture(&self) -> ResourceRef {
        self.texture.as_ref().unwrap().clone()
    }
    pub fn surface_texture(&self) -> Arc<wgpu::SurfaceTexture> {
        self.s.as_ref().unwrap().clone()
    }
}

impl Drop for WindowSurfaceFrame2 {
    fn drop(&mut self) {
        let t = self.texture.take().unwrap();
        self.gpu.context.deregister(t);
        if let Some(s) = self.s.take() {
            match Arc::try_unwrap(s) {
                Ok(v) => v.present(),
                Err(e) => {
                    log::error!("SurfaceFrame {:?}", e)
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct WGPUResource {
    device: Device,
    queue: Queue,
    instance: Arc<WGPUInstance>,
    context: Arc<RContext>,
    default_texture_id: ResourceRef,
    default_sampler_id: ResourceRef,
}

impl WGPUResource {
    pub fn default_texture(&self) -> ResourceRef {
        self.default_texture_id.clone()
    }

    pub fn default_sampler(&self) -> ResourceRef {
        self.default_sampler_id.clone()
    }

    fn build_surface_desc(width: u32, height: u32, format: TextureFormat) -> SurfaceConfiguration {
        SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        }
    }
    pub fn device(&self) -> &Device {
        &self.device
    }
    pub fn queue(&self) -> &Queue {
        &self.queue
    }
    pub(crate) fn surface(&self) -> &Surface {
        &self.instance.surface
    }
    pub fn current_frame_texture(&self) -> anyhow::Result<WindowSurfaceFrame> {
        let cp = self.surface().get_current_texture()?;
        let c = Arc::new(cp);
        let texture = self.context.register_surface_texture(c.clone());

        Ok(WindowSurfaceFrame {
            texture: Some(texture),
            s: Some(c),
            gpu: self,
        })
    }

    pub fn current_frame_texture2(self: Arc<Self>) -> anyhow::Result<WindowSurfaceFrame2> {
        let cp = self.surface().get_current_texture()?;
        let c = Arc::new(cp);
        let texture = self.context.register_surface_texture(c.clone());

        Ok(WindowSurfaceFrame2 {
            texture: Some(texture),
            s: Some(c),
            gpu: self.clone(),
        })
    }

    pub(crate) fn width(&self) -> u32 {
        let inner = self.instance.inner.lock().unwrap();
        inner.width
    }
    pub(crate) fn height(&self) -> u32 {
        let inner = self.instance.inner.lock().unwrap();
        inner.height
    }
    pub fn aspect(&self) -> f32 {
        let mut h = self.height();
        if h == 0 {
            h = 1;
        }
        self.width() as f32 / h as f32
    }
    pub(crate) fn set_width_height(&self, width: u32, height: u32) {
        let mut inner = self.instance.inner.lock().unwrap();
        inner.width = width;
        inner.height = height;
    }
    pub fn surface_format(&self) -> TextureFormat {
        self.instance.format
    }
    pub fn context(&self) -> &RContext {
        &self.context
    }
    pub fn context_ref(&self) -> RContextRef {
        self.context.clone()
    }

    pub(crate) fn new_queue(self: Arc<Self>) -> Arc<Self> {
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
    pub fn from_rgba_texture(&self, data: &[u8], size: Size) -> ResourceRef {
        let texture = self.device().create_texture_with_data(
            self.queue(),
            &wgpu::TextureDescriptor {
                label: Some("input"),
                size: wgpu::Extent3d {
                    width: size.x,
                    height: size.y,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            data,
        );

        self.context().register_texture(texture)
    }

    pub fn from_sampler(&self, desc: &wgpu::SamplerDescriptor) -> ResourceRef {
        let sampler = self.device().create_sampler(desc);

        self.context().register_sampler(sampler)
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
            bytes_per_row: Some(row_bytes),
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
            view_formats: &[],
        });
        texture
    }

    pub fn new_srgba_2d_texture(&self, label: Option<&'static str>, size: Size) -> wgpu::Texture {
        let device = self.device();
        device.create_texture(&wgpu::TextureDescriptor {
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
            view_formats: &[],
        })
    }

    pub(crate) fn new_2d_attachment_texture(
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
            view_formats: &[],
        });
        texture
    }

    pub fn new_sampler(&self, label: Option<&str>) -> wgpu::Sampler {
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
            anisotropy_clamp: 1,
            border_color: None,
        })
    }

    pub fn new_sampler_linear(&self, label: Option<&str>) -> wgpu::Sampler {
        self.device.create_sampler(&wgpu::SamplerDescriptor {
            label,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: 0f32,
            lod_max_clamp: f32::MAX,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        })
    }

    pub(crate) fn new_wvp_buffer<T>(&self, label: Option<&str>) -> wgpu::Buffer {
        self.device.create_buffer(&BufferDescriptor {
            label,
            size: std::mem::size_of::<T>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    pub fn new_wvp_buffer_from(&self, label: Option<&str>, data: &[u8]) -> wgpu::Buffer {
        self.device.create_buffer_init(&BufferInitDescriptor {
            label,
            usage: BufferUsages::UNIFORM,
            contents: data,
        })
    }

    pub(crate) fn new_uniform_buffer(&self, label: Option<&str>, size: u64) -> wgpu::Buffer {
        self.device.create_buffer(&BufferDescriptor {
            label,
            size,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
}

fn request_optional_feature(
    adapter: &wgpu::Adapter,
    features: wgpu::Features,
    limits: Limits,
    minimal_features: wgpu::Features,
    minimal_limits: Limits,
) -> (Device, Queue) {
    let req_features = adapter.features().intersection(features);
    if !req_features.contains(minimal_features) {
        panic!("unsupported features. current {:?}", req_features);
    }

    if !minimal_limits.check_limits(&adapter.limits()) {
        panic!("unsupported limits. current {:?}", adapter.limits());
    }

    if features != req_features {
        log::warn!("features downgrade {:?}", req_features);
    }

    let fut = adapter.request_device(
        &DeviceDescriptor {
            label: Some("wgpu device"),
            features: req_features,
            limits: adapter.limits(),
        },
        None,
    );
    match pollster::block_on(fut) {
        Ok(v) => return v,
        Err(err) => {
            log::error!("request device {}", err);
            panic!("request device fail");
        }
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

impl WGPUBackend {
    pub fn new<
        S: raw_window_handle::HasRawWindowHandle + raw_window_handle::HasRawDisplayHandle,
    >(
        surface: &S,
        width: u32,
        height: u32,
        context: RContextRef,
    ) -> Result<WGPUBackend> {
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
        log::info!("wgpu {:?}", bits);

        let instance = Instance::new(wgpu::InstanceDescriptor {
            backends: bits,
            dx12_shader_compiler: wgpu::Dx12Compiler::default(),
        });
        let surface = unsafe { instance.create_surface(surface) }.unwrap();
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
        let limits = wgpu::Limits {
            max_push_constant_size: 128,
            ..Default::default()
        };
        let minimal_limits = wgpu::Limits {
            max_push_constant_size: 128,
            ..Default::default()
        };

        let mut features = wgpu::Features::empty();
        features.insert(Features::PUSH_CONSTANTS);
        features.insert(Features::MULTI_DRAW_INDIRECT);
        features.insert(Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);
        features.insert(Features::ADDRESS_MODE_CLAMP_TO_BORDER);

        let mut minimal_features = wgpu::Features::empty();
        minimal_features.insert(Features::PUSH_CONSTANTS);
        minimal_features.insert(Features::ADDRESS_MODE_CLAMP_TO_BORDER);

        let (device, queue) =
            request_optional_feature(&adapter, features, limits, minimal_features, minimal_limits);

        let formats = surface.get_capabilities(&adapter).formats;

        let prefer_formats = &[TextureFormat::Bgra8Unorm, TextureFormat::Rgba8Unorm];
        log::info!("surface formats {:?}", &formats);

        let mut format = TextureFormat::R8Unorm;
        for f in prefer_formats {
            if formats.iter().find(|v| **v == *f).is_some() {
                format = *f;
                break;
            }
        }
        if format == TextureFormat::R8Unorm {
            anyhow::bail!("no texture format found")
        }

        let mut texture_data = vec![];
        let deep_color = Vec4::<u8>::new(44, 45, 40, 255);
        let white_color = Vec4::<u8>::new(192, 198, 202, 255);
        texture_data.push(deep_color);
        texture_data.push(white_color);
        texture_data.push(white_color);
        texture_data.push(deep_color);

        let default_texture = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("default texture"),
                size: wgpu::Extent3d {
                    width: 2,
                    height: 2,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            any_as_u8_slice_array(&texture_data),
        );

        let default_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("default sampler"),
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            lod_min_clamp: 0f32,
            lod_max_clamp: 20f32,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        });

        let default_texture_id = context.register_texture(default_texture);
        let default_sampler_id = context.register_sampler(default_sampler);

        // config first time

        surface.configure(
            &device,
            &WGPUResource::build_surface_desc(width, height, format),
        );

        Ok(WGPUBackend {
            inner: WGPUResource {
                context,
                default_texture_id,
                default_sampler_id,
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

#[derive(Debug, Clone)]
pub enum ClearValue {
    Color(crate::types::Color),
    Depth(f32),
    Stencil(u32),
    DepthAndStencil((f32, u32)),
}

impl ClearValue {
    pub fn depth(&self) -> Option<f32> {
        match self {
            ClearValue::Depth(d) => Some(*d),
            ClearValue::DepthAndStencil((d, s)) => Some(*d),
            _ => None,
        }
    }
    pub fn stencil(&self) -> Option<u32> {
        match self {
            ClearValue::Stencil(s) => Some(*s),
            ClearValue::DepthAndStencil((d, s)) => Some(*s),
            _ => None,
        }
    }
    pub fn color(&self) -> Option<crate::types::Color> {
        match self {
            ClearValue::Color(c) => Some(*c),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResourceOps {
    pub load: Option<ClearValue>,
    pub store: bool,
}

impl ResourceOps {
    pub fn load_store() -> Self {
        Self {
            load: None,
            store: true,
        }
    }
}

#[derive(Debug)]
pub struct WGPURenderer {
    pub(crate) inner: Arc<WGPUResource>,
    encoder: Option<CommandEncoder>,
    command_buffers: Vec<CommandBuffer>,
}

pub struct WGPURenderTargetInner<'a, 'b> {
    color_attachments: Vec<RenderPassColorAttachment<'a>>,
    depth_attachment: Option<RenderPassDepthStencilAttachment<'a>>,
    render_pass_desc: RenderPassDescriptor<'a, 'b>,
}

impl<'a, 'b> WGPURenderTargetInner<'a, 'b> {
    pub fn new(label: &'a str) -> Self {
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
            self.render_pass_desc.depth_stencil_attachment = self.depth_attachment.clone();
        }
        &self.render_pass_desc
    }
}

impl<'a, 'b> std::fmt::Debug for WGPURenderTargetInner<'a, 'b> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WGPURenderTargetInner")
            .field("color_attachments", &self.color_attachments)
            .field("depth_attachment", &self.depth_attachment)
            .finish()
    }
}

pub struct WGPURenderTarget {
    inner: std::ptr::NonNull<u8>,
    label: String,
}

impl std::fmt::Debug for WGPURenderTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.get();
        inner.fmt(f)
    }
}

unsafe impl std::marker::Send for WGPURenderTarget {}

impl WGPURenderTarget {
    pub fn new(label: &str) -> Self {
        let inner = Box::new(WGPURenderTargetInner::new(label));
        let ptr = Box::into_raw(inner);
        Self {
            inner: std::ptr::NonNull::new(ptr as *mut u8).unwrap(),
            label: label.into(),
        }
    }
    fn get<'a, 'b>(&self) -> &mut WGPURenderTargetInner<'a, 'b> {
        unsafe { std::mem::transmute(self.inner.as_ptr()) }
    }

    pub fn desc<'a, 'b>(&self) -> &RenderPassDescriptor<'a, 'b> {
        self.get().desc()
    }

    fn map_ops(color: Option<ResourceOps>) -> Operations<wgpu::Color> {
        if let Some(color) = color {
            Operations {
                load: color.load.map_or(LoadOp::Load, |c| {
                    let c = c.color().unwrap();
                    LoadOp::Clear(wgpu::Color {
                        r: c.x as f64,
                        g: c.y as f64,
                        b: c.z as f64,
                        a: c.w as f64,
                    })
                }),
                store: color.store,
            }
        } else {
            Operations {
                load: LoadOp::Load,
                store: false,
            }
        }
    }

    pub fn set_depth_target(
        &mut self,
        view: &TextureView,
        clear: Option<ResourceOps>,
        clear_stencil: Option<ResourceOps>,
    ) {
        let inner = self.get();
        let ops = if let Some(clear) = clear {
            Some(Operations {
                load: clear.load.map_or(wgpu::LoadOp::Load, |v| {
                    wgpu::LoadOp::Clear(v.depth().unwrap())
                }),
                store: clear.store,
            })
        } else {
            None
        };
        let ops_stencil = if let Some(clear_stencil) = clear_stencil {
            Some(Operations {
                load: clear_stencil.load.map_or(wgpu::LoadOp::Load, |v| {
                    wgpu::LoadOp::Clear(v.stencil().unwrap())
                }),
                store: clear_stencil.store,
            })
        } else {
            None
        };
        inner.depth_attachment = Some(RenderPassDepthStencilAttachment {
            view,
            depth_ops: ops,
            stencil_ops: ops_stencil,
        });
    }

    // pub fn set_render_target(&mut self, texture_view: &TextureView, color: Option<ResourceOps>) {
    //     let inner = self.get();
    //     let ops = Self::map_ops(color);
    //     if inner.color_attachments.is_empty() {
    //         inner.color_attachments.push(RenderPassColorAttachment {
    //             view: texture_view,
    //             resolve_target: None,
    //             ops,
    //         })
    //     } else {
    //         inner.color_attachments[0] = RenderPassColorAttachment {
    //             view: texture_view,
    //             resolve_target: None,
    //             ops,
    //         }
    //     }
    // }

    pub fn add_render_target(&mut self, texture_view: &TextureView, color: Option<ResourceOps>) {
        let inner = self.get();
        let ops = Self::map_ops(color);
        inner.color_attachments.push(RenderPassColorAttachment {
            view: texture_view,
            resolve_target: None,
            ops,
        })
    }

    pub fn add_resolved_render_target(
        &mut self,
        texture_view: &TextureView,
        rtv: Option<&TextureView>,
        color: Option<ResourceOps>,
    ) {
        let inner = self.get();
        let ops = Self::map_ops(color);
        inner.color_attachments.push(RenderPassColorAttachment {
            view: texture_view,
            resolve_target: rtv,
            ops,
        })
    }
}

impl Drop for WGPURenderTarget {
    fn drop(&mut self) {
        unsafe {
            let _ = Box::from_raw(self.inner.as_mut());
        }
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
        }
    }
    pub fn resource(&self) -> Arc<WGPUResource> {
        self.inner.clone()
    }

    pub fn encoder(&self) -> &wgpu::CommandEncoder {
        self.encoder.as_ref().unwrap()
    }
    pub fn encoder_mut(&mut self) -> &mut wgpu::CommandEncoder {
        self.encoder.as_mut().unwrap()
    }
}

impl WGPURenderer {
    pub fn new_pass<'b>(&'b mut self, target: &'b WGPURenderTarget) -> RenderPass<'b> {
        let encoder = self.encoder.as_mut().unwrap();
        encoder.begin_render_pass(target.desc())
    }
}

impl Drop for WGPURenderer {
    fn drop(&mut self) {
        self.command_buffers
            .push(self.encoder.take().unwrap().finish());

        let mut tmp = Vec::new();

        std::mem::swap(&mut tmp, &mut self.command_buffers);

        self.inner.queue.submit(tmp);
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
    fn on_event(&mut self, source: &dyn EventSource, event: &dyn Any) -> ProcessEventResult {
        if let Some(event) = event.downcast_ref::<Event>() {
            match event {
                Event::Resized {
                    physical,
                    logical: _,
                } => {
                    let width = u32::max(physical.x, 16);
                    let height = u32::max(physical.y, 16);
                    let format = self.format;

                    self.inner.surface().configure(
                        &self.inner.device,
                        &WGPUResource::build_surface_desc(width, height, format),
                    );
                    self.inner.set_width_height(width, height);
                    let _ = source
                        .event_sender()
                        .send_event(Box::new(Event::JustRenderOnce));
                }
                _ => (),
            };
        }
        ProcessEventResult::Received
    }
}

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
            label,
            size,
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

    pub fn prepare(&mut self, mut size: u64, gpu: &WGPUResource) -> bool {
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
        true
    }
}

pub struct NullBufferAccessor;

impl<'a> BufferAccessor<'a> for NullBufferAccessor {
    fn buffer_slice(&self, id: u64, range: Range<u64>) -> Option<wgpu::BufferSlice<'a>> {
        None
    }
}

pub struct GpuInputMainBuffer {
    buffer: GpuMainBuffer,
    stage: StagingBelt,
    chunk_size: u64,
    alignment: u64,
    offset: u64,
}

impl<'a> BufferAccessor<'a> for &'a GpuInputMainBuffer {
    fn buffer_slice(&self, _id: u64, range: Range<u64>) -> Option<wgpu::BufferSlice<'a>> {
        Some(self.buffer.buffer().slice(range))
    }
}

impl GpuInputMainBuffer {
    pub fn new(gpu: &WGPUResource, label: Option<&'static str>, usage: wgpu::BufferUsages) -> Self {
        let chunk_size = 1024 * 1024 * 2; // 2 Mib

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
    pub fn prepare(&mut self, gpu: &WGPUResource, bytes: u64) -> bool {
        self.buffer.prepare(bytes + self.offset, gpu)
    }

    pub fn copy_stage(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        gpu: &WGPUResource,
        data: &[u8],
    ) -> Range<u64> {
        let size = data.len() as u64;
        if size == 0 {
            return self.offset..self.offset;
        }

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

// pub struct GpuInputUniformBuffers {
//     buffers: Vec<(wgpu::Buffer, wgpu::BindGroup)>,

//     label: Option<&'static str>,
//     stage: StagingBelt,

//     recent_usage_size: VecDeque<u64>,
//     tick: u64,

//     index: u64,
//     offset: u64,
//     size: u64,
//     alignment: u64,
// }

// impl GpuInputUniformBuffers {
//     pub fn new_with(gpu: &WGPUResource, label: Option<&'static str>) -> Self {
//         let size = gpu.device.limits().max_uniform_buffer_binding_size;
//         let alignment = gpu.device.limits().min_uniform_buffer_offset_alignment;

//         Self {
//             buffers: Vec::new(),
//             label,
//             stage: StagingBelt::new(size as u64),

//             recent_usage_size: VecDeque::new(),
//             tick: 0,
//             index: 0,
//             offset: 0,

//             size: size as u64,
//             alignment: alignment as u64,
//         }
//     }

//     #[inline]
//     pub fn recall(&mut self) {
//         self.stage.recall();
//         self.index = 0;
//         self.offset = 0;
//     }

//     #[inline]
//     pub fn finish(&mut self) {
//         self.stage.finish();
//     }

//     #[inline]
//     pub fn prepare(&mut self, gpu: &WGPUResource, n: u64, mut single_bytes: u64) {
//         single_bytes = (single_bytes + self.alignment - 1) & (self.alignment - 1).not();

//         let elements_for_uniform_buffer = self.size / single_bytes;
//         assert!(elements_for_uniform_buffer != 0);
//         let total_buffers =
//             (n + elements_for_uniform_buffer + 1) / elements_for_uniform_buffer + (self.index + 1);

//         while self.buffers.len() < total_buffers as usize {
//             let buffer = gpu.device().create_buffer(&wgpu::BufferDescriptor {
//                 label: self.label,
//                 size: self.size,
//                 usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
//                 mapped_at_creation: false,
//             });
//             let bind_group = (self.bind_group_creator)(&buffer, gpu);

//             self.buffers.push((buffer, bind_group));
//         }
//     }

//     pub fn copy_stage(
//         &mut self,
//         encoder: &mut wgpu::CommandEncoder,
//         gpu: &WGPUResource,
//         data: &[u8],
//     ) -> BufferPosition {
//         let rest = self.size - self.offset;
//         let size = data.len() as u64;

//         if rest < size {
//             self.index += 1;
//             self.offset = 0;
//         }

//         let mut bytes = self.stage.write_buffer(
//             encoder,
//             &self.buffers[self.index as usize].0,
//             self.offset,
//             NonZeroU64::new(size).unwrap(),
//             gpu.device(),
//         );
//         bytes.copy_from_slice(data);

//         let range = self.offset..(self.offset + size);
//         self.offset = (range.end + self.alignment - 1) & (self.alignment - 1).not();

//         BufferPosition {
//             index: self.index,
//             range,
//         }
//     }
// }

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

    pub fn prepare(&mut self, gpu: &WGPUResource, index_bytes: u64, vertex_bytes: u64) {
        self.index.prepare(gpu, index_bytes);
        self.vertex.prepare(gpu, vertex_bytes);
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

    pub fn vertex(&self) -> &GpuInputMainBuffer {
        &self.vertex
    }
    pub fn index(&self) -> &GpuInputMainBuffer {
        &self.index
    }
}

pub struct GpuInputMainBuffersWithProps {
    index: GpuInputMainBuffer,
    vertex: GpuInputMainBuffer,
    vertex_props: GpuInputMainBuffer,
}

impl GpuInputMainBuffersWithProps {
    pub fn new(gpu: &WGPUResource, label: Option<&'static str>) -> Self {
        Self {
            index: GpuInputMainBuffer::new(gpu, label, wgpu::BufferUsages::INDEX),
            vertex: GpuInputMainBuffer::new(gpu, label, wgpu::BufferUsages::VERTEX),
            vertex_props: GpuInputMainBuffer::new(gpu, label, wgpu::BufferUsages::VERTEX),
        }
    }

    pub fn prepare(
        &mut self,
        gpu: &WGPUResource,
        index_bytes: u64,
        vertex_bytes: u64,
        vertex_props_bytes: u64,
    ) {
        self.index.prepare(gpu, index_bytes);
        self.vertex.prepare(gpu, vertex_bytes);
        self.vertex_props.prepare(gpu, vertex_props_bytes);
    }

    pub fn copy_stage(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        gpu: &WGPUResource,
        indices: &[u8],
        vertices: &[u8],
        vertices_props: &[u8],
    ) -> (Range<u64>, Range<u64>, Range<u64>) {
        let index = self.index.copy_stage(encoder, gpu, indices);
        let vertex = self.vertex.copy_stage(encoder, gpu, vertices);
        let vertex_props = self.vertex_props.copy_stage(encoder, gpu, vertices_props);
        (index, vertex, vertex_props)
    }

    pub fn recall(&mut self) {
        self.index.recall();
        self.vertex.recall();
        self.vertex_props.recall();
    }

    pub fn finish(&mut self) {
        self.index.finish();
        self.vertex.finish();
        self.vertex_props.finish();
    }

    pub fn vertex(&self) -> &GpuInputMainBuffer {
        &self.vertex
    }
    pub fn vertex_props(&self) -> &GpuInputMainBuffer {
        &self.vertex_props
    }
    pub fn index(&self) -> &GpuInputMainBuffer {
        &self.index
    }
}

// pub struct GpuInputMainBuffersWithUniform {
//     index: GpuInputMainBuffer,
//     vertex: GpuInputMainBuffer,
//     uniform: GpuInputUniformBuffers,
// }

pub struct BufferPosition {
    pub index: u64,
    pub range: Range<u64>,
}

// impl GpuInputMainBuffersWithUniform {
//     pub fn new(gpu: &WGPUResource, label: Option<&'static str>) -> Self {
//         let uniform = GpuInputUniformBuffers::new(gpu, label);

//         Self {
//             index: GpuInputMainBuffer::new(gpu, label, wgpu::BufferUsages::INDEX),
//             vertex: GpuInputMainBuffer::new(gpu, label, wgpu::BufferUsages::VERTEX),
//             uniform,
//         }
//     }

//     pub fn prepare(
//         &mut self,
//         gpu: &WGPUResource,
//         index_bytes: u64,
//         vertex_bytes: u64,
//         n_uniform: u64,
//         single_bytes: u64,
//     ) -> bool {
//         self.index.prepare(gpu, index_bytes);
//         self.vertex.prepare(gpu, vertex_bytes);
//         let changed = self.uniform.prepare(gpu, n_uniform, single_bytes);
//         changed
//     }

//     pub fn copy_stage(
//         &mut self,
//         encoder: &mut wgpu::CommandEncoder,
//         gpu: &WGPUResource,
//         indices: &[u8],
//         vertices: &[u8],
//         uniforms: &[u8],
//     ) -> (Range<u64>, Range<u64>, BufferPosition) {
//         let index = self.index.copy_stage(encoder, gpu, indices);
//         let vertex = self.vertex.copy_stage(encoder, gpu, vertices);
//         let uniform = self.uniform.copy_stage(encoder, gpu, uniforms);
//         (index, vertex, uniform)
//     }

//     pub fn recall(&mut self) {
//         self.index.recall();
//         self.vertex.recall();
//         self.uniform.recall();
//     }

//     pub fn finish(&mut self) {
//         self.index.finish();
//         self.vertex.finish();
//         self.uniform.finish();
//     }

//     pub fn vertex_buffer(&self) -> &wgpu::Buffer {
//         self.vertex.buffer()
//     }
//     pub fn vertex_buffer_slice(&self, range: Range<u64>) -> wgpu::BufferSlice {
//         self.vertex.buffer().slice(range)
//     }
//     pub fn index_buffer(&self) -> &wgpu::Buffer {
//         self.index.buffer()
//     }
//     pub fn index_buffer_slice(&self, range: Range<u64>) -> wgpu::BufferSlice {
//         self.index.buffer().slice(range)
//     }

//     pub fn uniform_buffer(&self, buffer_position: BufferPosition) -> &wgpu::Buffer {
//         self.uniform.buffer(buffer_position.index)
//     }
//     pub fn uniform_buffers(&self) -> &[wgpu::Buffer] {
//         self.uniform.buffers()
//     }
// }
