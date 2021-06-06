use std::{cell::RefCell, sync::Mutex};

use winit::event::WindowEvent;

use crate::{
    renderer::{RenderObject, UpdateContext},
    types::*,
};
struct Inner {
    pixels_horizontal: u32,
    pixels_vertical: u32,
    buf: Vec<u32>,
}

impl Inner {}

struct CanvasPosition {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
}

pub struct Canvas {
    inner: Mutex<Inner>,
    current_inner: RefCell<Inner>,
    position: Mutex<CanvasPosition>,
}

impl Canvas {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                buf: Vec::new(),
                pixels_horizontal: 10,
                pixels_vertical: 10,
            }),
            current_inner: RefCell::new(Inner {
                buf: Vec::new(),
                pixels_horizontal: 10,
                pixels_vertical: 10,
            }),
            position: Mutex::new(CanvasPosition {
                left: 100,
                top: 5,
                width: 100,
                height: 100,
            }),
        }
    }

    fn resize(&self, width: u32, height: u32) {
        let mut p = self.position.lock().unwrap();
        p.height = height;
        p.width = width;
    }

    pub fn set_position(&self, x: u32, y: u32) {
        let mut p = self.position.lock().unwrap();
        p.left = x;
        p.top = y;
    }

    pub fn on_event(&self, event: &WindowEvent) {
        match event {
            WindowEvent::Resized(size) => {
                self.resize(size.width, size.height);
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                new_inner_size,
            } => {
                let size = new_inner_size;
                self.resize(size.width, size.height);
            }
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
                modifiers,
            } => {}
            _ => (),
        };
    }

    pub fn async_clear(&self, color: Color) {
        self.current_inner.borrow_mut().buf.fill(color.to_argb());
    }

    pub fn async_draw_pixel(&self, pos: Position2, color: Color) {
        let inner = self.inner.lock().unwrap();
        unsafe {
            *self.current_inner.borrow_mut().buf.get_unchecked_mut(
                (pos.y * self.current_inner.borrow().pixels_horizontal + pos.x) as usize,
            ) = color.to_argb();
        }
    }
}

impl RenderObject for Canvas {
    fn render<'a>(&'a mut self, pass: &mut wgpu::RenderPass<'a>) {}

    fn init_renderer(&mut self, device: &mut wgpu::Device) {}

    fn on_event(&mut self, event: &WindowEvent) {}

    fn update<'a>(&'a mut self, ctx: UpdateContext<'a>) -> bool {
        false
    }

    fn prepare_render<'a>(&'a mut self, ctx: crate::renderer::RenderContext<'a>) {}
}
