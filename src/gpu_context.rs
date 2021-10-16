use crate::types::Size;
use parking_lot::Mutex;
use std::cell::RefCell;
use std::sync::Arc;
use wgpu::*;
use winit::window::{Window, WindowId};

#[derive(Debug)]
pub struct GpuInstance {
    surface: Surface,
    adapter: Adapter,
    device: Device,
    queue: Queue,
    id: WindowId,
    main_id: WindowId,
}

impl GpuInstance {
    pub fn device(&self) -> &Device {
        &self.device
    }
    pub fn queue(&self) -> &Queue {
        &self.queue
    }
    pub fn surface(&self) -> &Surface {
        &self.surface
    }
    pub fn id(&self) -> WindowId {
        self.id
    }
    pub fn main_id(&self) -> WindowId {
        self.main_id
    }
    pub fn is_main_window(&self) -> bool {
        self.id == self.main_id
    }
}

pub type GpuInstanceRef = Arc<GpuInstance>;

#[derive(Debug)]
pub struct GpuContext {
    instance: Instance,
    main_id: Mutex<Option<WindowId>>,
}

thread_local! {
    static WINDOW_CTX: RefCell<Option<GpuInstanceRef>> = RefCell::new(None);
}

pub type GpuContextRef = Arc<GpuContext>;

#[derive(Debug)]
pub struct GpuAttachResource {
    surface: Surface,
    size: Size,
    id: WindowId,
}

impl GpuContext {
    fn build_surface_desc(width: u32, height: u32) -> SurfaceConfiguration {
        SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: TextureFormat::Rgba8UnormSrgb,
            width,
            height,
            present_mode: wgpu::PresentMode::Immediate,
        }
    }
    pub fn new() -> Self {
        let bits = wgpu::util::backend_bits_from_env().unwrap_or(wgpu::Backends::PRIMARY);
        let instance = Instance::new(bits);
        Self {
            instance,
            main_id: None.into(),
        }
    }
    pub fn attach_window(&self, window: &Window) -> GpuAttachResource {
        let surface = unsafe { self.instance.create_surface(window) };
        let size = window.inner_size();
        let size = Size::new(size.width, size.height);
        let mut guard = self.main_id.lock();
        if guard.is_none() {
            *guard = Some(window.id());
        }
        GpuAttachResource {
            surface,
            size,
            id: window.id(),
        }
    }

    pub async fn attach_resource(&self, resource: GpuAttachResource) {
        let adapter = self
            .instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::default(),
                compatible_surface: Some(&resource.surface),
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
        let width = resource.size.x;
        let height = resource.size.y;
        resource
            .surface
            .configure(&device, &Self::build_surface_desc(width, height));
        // let format = adapter.get_swap_chain_preferred_format(&surface).unwrap();
        let id = resource.id;
        let ctx = GpuInstance {
            surface: resource.surface,
            adapter,
            device,
            queue,
            id,
            main_id: self.main_id.lock().unwrap(),
        }
        .into();
        WINDOW_CTX.with(|wctx| *wctx.borrow_mut() = Some(ctx));
    }

    pub fn detach(&self) {
        WINDOW_CTX.with(|wctx| *wctx.borrow_mut() = None);
    }

    pub fn instance(&self) -> GpuInstanceRef {
        let ctx = WINDOW_CTX.with(|wctx| wctx.borrow().clone());
        ctx.unwrap()
    }

    pub fn rebuild(&self, size: Size) {
        let ctx = self.instance();
        ctx.surface
            .configure(&ctx.device, &Self::build_surface_desc(size.x, size.y));
    }
}
