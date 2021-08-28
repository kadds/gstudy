use crate::{statistics::Statistics, UserEvent};
use std::time::{Duration, Instant};
use wgpu::*;
use winit::{dpi::PhysicalSize, event::WindowEvent, window::Window};

#[derive(Debug)]
pub struct RenderContext<'a> {
    pub queue: &'a mut Queue,
    pub device: &'a mut Device,
    pub encoder: &'a mut CommandEncoder,
}

#[derive(Debug)]
pub struct UpdateContext<'a> {
    pub update_statistics: &'a Statistics,
    pub render_statistics: &'a Statistics,
}

pub trait RenderObject {
    fn update<'a>(&'a mut self, ctx: UpdateContext<'a>) -> bool;
    fn prepare_render<'a>(&'a mut self, ctx: RenderContext<'a>);
    fn render<'a>(&'a mut self, pass: &mut RenderPass<'a>);
    fn init_renderer(&mut self, device: &mut Device);
    fn on_event(&mut self, event: &WindowEvent);
    fn on_user_event(&mut self, event: &UserEvent);
    fn zlevel(&self) -> i64;
}

pub struct Renderer {
    instance: Instance,
    surface: Surface,
    adapter: Adapter,
    device: Device,
    queue: Queue,
    objects: Vec<Box<dyn RenderObject>>,
    render_statistics: Statistics,
    update_statistics: Statistics,
    clear_color: Option<Color>,
    width: u32,
    height: u32,
}

impl Renderer {
    fn build_surface_desc(width: u32, height: u32) -> SurfaceConfiguration {
        SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: TextureFormat::Bgra8UnormSrgb,
            width,
            height,
            present_mode: wgpu::PresentMode::Immediate,
        }
    }
    pub async fn new(window: &Window) -> Renderer {
        let bits = wgpu::util::backend_bits_from_env().unwrap_or(wgpu::Backends::PRIMARY);
        let instance = Instance::new(bits);
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::default(),
                compatible_surface: Some(&surface),
            })
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    features: Features::empty(),
                    limits: Limits::default(),
                    label: None,
                },
                None,
            )
            .await
            .unwrap();
        let wsize = window.inner_size();
        let width = wsize.width;
        let height = wsize.height;
        surface.configure(&device, &Self::build_surface_desc(width, height));

        // let format = adapter.get_swap_chain_preferred_format(&surface).unwrap();
        Self {
            instance,
            surface,
            adapter,
            device,
            queue,
            objects: Vec::new(),
            render_statistics: Statistics::new(Duration::from_millis(900), Some(1f32 / 1000f32)),
            update_statistics: Statistics::new(Duration::from_millis(900), Some(1f32 / 1000f32)),
            clear_color: Some(Color::BLACK),
            height,
            width,
        }
    }

    pub fn set_frame_lock(&mut self, target_frame_secends: Option<f32>) {
        self.render_statistics.set_frame_lock(target_frame_secends);
    }
    pub fn set_update_frame_lock(&mut self, target_frame_secends: Option<f32>) {
        self.update_statistics.set_frame_lock(target_frame_secends);
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;
        self.surface
            .configure(&self.device, &Self::build_surface_desc(width, height));
    }

    pub fn update(&mut self) -> (Instant, bool) {
        self.update_statistics.new_frame();
        let mut need_render = false;
        if !self.objects.is_empty() {
            for r in self.objects.iter_mut() {
                if r.update(UpdateContext {
                    update_statistics: &self.update_statistics,
                    render_statistics: &self.render_statistics,
                }) {
                    need_render = true;
                }
            }
        } else {
            need_render = true;
        }
        (self.update_statistics.get_waiting(), need_render)
    }

    pub fn render(&mut self) -> Instant {
        self.render_statistics.new_frame();
        {
            let frame = match self.surface.get_current_frame() {
                Ok(v) => v.output,
                Err(e) => {
                    log::error!("get swapchine fail. {}", e);
                    return Instant::now();
                }
            };

            let mut encoder = self
                .device
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("GStudy encoder"),
                });
            for r in self.objects.iter_mut() {
                r.prepare_render(RenderContext {
                    queue: &mut self.queue,
                    device: &mut self.device,
                    encoder: &mut encoder,
                })
            }
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
                for r in self.objects.iter_mut() {
                    r.render(&mut render_pass);
                }
            }
            self.queue.submit(std::iter::once(encoder.finish()));
        }
        self.render_statistics.get_waiting()
    }

    pub fn add(&mut self, mut obj: Box<dyn RenderObject>) {
        obj.init_renderer(&mut self.device);
        obj.on_event(&WindowEvent::Resized(PhysicalSize::new(
            self.width,
            self.height,
        )));
        let idx = self.objects.partition_point(|o| o.zlevel() < obj.zlevel());
        self.objects.insert(idx, obj);
    }

    pub fn set_clear_color(&mut self, color: Option<Color>) {
        self.clear_color = color;
    }

    pub fn on_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::Resized(size) => {
                self.resize(size.width, size.height);
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor: _,
                new_inner_size,
            } => {
                self.resize(new_inner_size.width, new_inner_size.height);
            }
            _ => (),
        }

        for r in self.objects.iter_mut() {
            r.on_event(event);
        }
    }
    pub fn on_user_event(&mut self, event: &UserEvent) {
        for r in self.objects.iter_mut() {
            r.on_user_event(event);
        }
    }
}
