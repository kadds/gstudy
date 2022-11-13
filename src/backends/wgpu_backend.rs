use std::sync::{atomic::AtomicPtr, Arc, Mutex};

use crate::{
    event::{Event, EventProcessor, EventSource, ProcessEventResult},
    types::Size,
};
use anyhow::{anyhow, Result};
use wgpu::*;

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
}

#[derive(Debug)]
pub struct WGPUResource {
    device: Device,
    queue: Queue,
    instance: Arc<WGPUInstance>,
}

impl WGPUResource {
    fn build_surface_desc(width: u32, height: u32) -> SurfaceConfiguration {
        SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: TextureFormat::Rgba8Unorm,
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

pub struct WGPUBackend {
    inner: Arc<WGPUResource>,
}

pub struct WGPUEventProcessor {
    inner: Arc<WGPUResource>,
}

impl WGPUBackend {
    pub fn new(window: &winit::window::Window) -> Result<WGPUBackend> {
        let bits = wgpu::util::backend_bits_from_env().unwrap_or(wgpu::Backends::PRIMARY);
        let instance = Instance::new(bits);
        let surface = unsafe { instance.create_surface(window) };
        let adapter_fut = instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        });
        let adapter_fut2 = instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::LowPower,
            force_fallback_adapter: true,
            compatible_surface: Some(&surface),
        });
        #[cfg(not(target_arch = "wasm32"))]
        let adapter = {
            match pollster::block_on(adapter_fut) {
                Some(v) => v,
                None => {
                    // fallback to adapter config 2
                    pollster::block_on(adapter_fut2).ok_or_else(|| anyhow!("no adapter found"))?
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
        #[cfg(not(target_arch = "wasm32"))]
        let (device, queue) = pollster::block_on(device_fut)?;

        Ok(WGPUBackend {
            inner: WGPUResource {
                instance: Arc::new(WGPUInstance {
                    instance,
                    surface,
                    adapter,
                    inner: Mutex::new(WGPUResourceInner {
                        width: 0,
                        height: 0,
                    }),
                }),
                device,
                queue,
            }
            .into(),
        })
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
    frame: Option<WGPUFrame>,
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
            frame: None,
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
    pub fn begin_surface<'a>(
        &'a mut self,
        render_target: &'a mut WGPURenderTarget,
        clear_color: Option<crate::types::Color>,
    ) -> Option<PassEncoder<'a>> {
        let frame = match self.inner.surface().get_current_texture() {
            Ok(v) => v,
            Err(e) => {
                log::error!("get swapchain fail {}", e);
                return None;
            }
        };
        let texture_view = frame.texture.create_view(&TextureViewDescriptor::default());
        self.frame = Some(WGPUFrame {
            frame,
            frame_texture_view: texture_view,
        });
        render_target.set_render_target(
            &self.frame.as_ref().unwrap().frame_texture_view,
            clear_color,
        );

        if self.first_call {
            render_target.reset();
            self.first_call = false;
        }

        Some(PassEncoder {
            renderer: self,
            render_target,
        })
    }

    pub fn begin_surface_with_depth<'a>(
        &'a mut self,
        render_target: &'a mut WGPURenderTarget,
        clear_color: Option<crate::types::Color>,
        depth_view: &TextureView,
        depth_clear: Option<f32>,
    ) -> Option<PassEncoder<'a>> {
        render_target.set_depth_target(depth_view, depth_clear);

        self.begin_surface(render_target, clear_color)
    }

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
        if let Some(sr) = self.frame.take() {
            sr.frame.present();
        }
    }
}

impl WGPUBackend {
    pub fn event_processor(&self) -> Box<dyn EventProcessor> {
        Box::new(WGPUEventProcessor {
            inner: self.inner.clone(),
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
            Event::Resized(_) => {
                let size = source.window().inner_size();
                let width = u32::max(size.width, 16);
                let height = u32::max(size.height, 16);

                self.inner.surface().configure(
                    &self.inner.device,
                    &WGPUResource::build_surface_desc(width, height),
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
