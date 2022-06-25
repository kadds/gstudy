use std::{
    borrow::BorrowMut,
    cell::{Ref, RefCell},
    rc::Rc,
};

use crate::event::{Event, EventProcessor, EventSource, ProcessEventResult};
use anyhow::{anyhow, Result};
use ouroboros::self_referencing;
use wgpu::*;

#[derive(Debug)]
struct WGPUResourceInner {
    width: u32,
    height: u32,
}

#[derive(Debug)]
pub struct WGPUResource {
    instance: Instance,
    surface: Surface,
    adapter: Adapter,
    device: Device,
    queue: Queue,
    inner: RefCell<WGPUResourceInner>,
}

impl WGPUResource {
    fn build_surface_desc(width: u32, height: u32) -> SurfaceConfiguration {
        SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: TextureFormat::Rgba8UnormSrgb,
            width,
            height,
            present_mode: wgpu::PresentMode::Immediate,
        }
    }
    pub fn device(&self) -> &Device {
        &self.device
    }
    pub fn queue(&self) -> &Queue {
        &self.queue
    }
    pub fn width(&self) -> u32 {
        let inner = self.inner.borrow();
        inner.width
    }
    pub fn height(&self) -> u32 {
        let inner = self.inner.borrow();
        inner.height
    }
}

pub struct WGPUBackend {
    inner: Rc<WGPUResource>,
}

pub struct WGPUEventProcessor {
    inner: Rc<WGPUResource>,
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
        #[cfg(not(target_arch = "wasm32"))]
        let adapter = pollster::block_on(adapter_fut).ok_or(anyhow!("no adapter found"))?;

        let device_fut = adapter.request_device(
            &DeviceDescriptor {
                features: Features::empty(),
                limits: Limits::default(),
                label: Some("wgpu_desc"),
            },
            None,
        );
        #[cfg(not(target_arch = "wasm32"))]
        let (device, queue) = pollster::block_on(device_fut)?;

        Ok(WGPUBackend {
            inner: WGPUResource {
                instance,
                surface,
                adapter,
                device,
                queue,
                inner: RefCell::new(WGPUResourceInner {
                    width: 0,
                    height: 0,
                }),
            }
            .into(),
        })
    }
}

#[self_referencing]
struct WGPUSurfaceRenderer {
    frame: Option<SurfaceTexture>,
    texture_view: TextureView,
    #[borrows(texture_view)]
    #[covariant]
    color_attachments: [RenderPassColorAttachment<'this>; 1],
    #[borrows(color_attachments)]
    #[covariant]
    desc: RenderPassDescriptor<'this, 'this>,
}

pub struct WGPURenderer {
    inner: Rc<WGPUResource>,
    encoder: Option<CommandEncoder>,
    surface_renderer: Option<WGPUSurfaceRenderer>,
}

pub struct PassEncoder<'a> {
    renderer: &'a mut WGPURenderer,
}

impl<'a> PassEncoder<'a> {
    pub fn new_pass(&'a mut self) -> RenderPass<'a> {
        let encoder = self.renderer.encoder.as_mut().unwrap();
        let surface = self.renderer.surface_renderer.as_ref().unwrap();
        encoder.begin_render_pass(surface.borrow_desc())
    }
}

impl WGPURenderer {
    pub fn begin_surface<'a>(
        &'a mut self,
        clear_color: Option<crate::types::Color>,
    ) -> Option<PassEncoder<'a>> {
        let frame = match self.inner.surface.get_current_texture() {
            Ok(v) => v,
            Err(e) => {
                log::error!("get swapchain fail {}", e);
                return None;
            }
        };
        let texture_view = frame.texture.create_view(&TextureViewDescriptor::default());
        let surface_renderer = WGPUSurfaceRendererBuilder {
            frame: Some(frame),
            texture_view,
            color_attachments_builder: |view: &TextureView| {
                [RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: Operations {
                        load: clear_color.map_or(LoadOp::Load, |v| {
                            LoadOp::Clear(Color {
                                r: v.x as f64,
                                g: v.y as f64,
                                b: v.z as f64,
                                a: v.w as f64,
                            })
                        }),
                        store: true,
                    },
                }]
            },
            desc_builder: |color_attachments: &[RenderPassColorAttachment; 1]| {
                RenderPassDescriptor {
                    label: None,
                    color_attachments: color_attachments,
                    depth_stencil_attachment: None,
                }
            },
        }
        .build();
        self.surface_renderer = Some(surface_renderer);
        Some(PassEncoder { renderer: self })
    }

    pub fn resource(&self) -> Rc<WGPUResource> {
        self.inner.clone()
    }
}

impl Drop for WGPURenderer {
    fn drop(&mut self) {
        self.inner
            .queue
            .submit(std::iter::once(self.encoder.take().unwrap().finish()));
        if let Some(mut sr) = self.surface_renderer.take() {
            sr.with_frame_mut(|f| {
                f.take().unwrap().present();
            });
        }
    }
}

impl WGPUBackend {
    pub fn event_processor(&self) -> Box<dyn EventProcessor> {
        Box::new(WGPUEventProcessor {
            inner: self.inner.clone(),
        })
    }

    pub fn renderer(&self) -> WGPURenderer {
        let encoder = self
            .inner
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("wgpu encoder"),
            });
        WGPURenderer {
            inner: self.inner.clone(),
            encoder: Some(encoder),
            surface_renderer: None,
        }
    }
}

impl EventProcessor for WGPUEventProcessor {
    fn on_event(&mut self, source: &dyn EventSource, event: &Event) -> ProcessEventResult {
        match event {
            Event::Resized(_) => {
                let size = source.window().inner_size();
                let width = u32::max(size.width, 16);
                let height = u32::max(size.height, 16);

                self.inner.surface.configure(
                    &self.inner.device,
                    &WGPUResource::build_surface_desc(width, height),
                );
                let mut inner = self.inner.inner.borrow_mut();
                inner.width = width;
                inner.height = height;
                let _ = source.event_proxy().send_event(Event::JustRenderOnce);
            }
            Event::Render => {}
            _ => (),
        };
        ProcessEventResult::Received
    }
}
