use std::sync::Mutex;

use atomic::{Atomic, Ordering};
use egui::{CtxRef, InnerResponse, Ui, Window};
use winit::{event_loop::EventLoopProxy, window::WindowId};

use crate::{
    gpu_context::GpuInstance,
    render_window::{GlobalUserEvent, NewWindowProps, UserEvent},
    types::{Size, Vec2f},
};

#[derive(Debug, Clone, Copy)]
pub struct WindowInfo {
    pub pos: Vec2f,
    pub size: Size,
    pub window_id: Option<WindowId>,
    pub logic_window_id: u64,
}

pub struct SubWindowUIState<U: Sized, T: Sized + Copy> {
    pub inner: Mutex<U>,
    pub inner_shared: Atomic<T>,
    pub info: Atomic<WindowInfo>,
}

impl<U: Sized, T: Sized + Copy> SubWindowUIState<U, T> {
    pub fn new(logic_window_id: u64, inner: U, inner_shared: T) -> Self {
        Self {
            inner_shared: Atomic::new(inner_shared),
            inner: inner.into(),
            info: Atomic::new(WindowInfo {
                pos: Vec2f::new(0f32, 0f32),
                size: Size::new(10, 10),
                window_id: None,
                logic_window_id,
            }),
        }
    }

    pub fn load_shared(&self) -> T {
        self.inner_shared.load(Ordering::Acquire)
    }

    pub fn load_shared_weak(&self) -> T {
        self.inner_shared.load(Ordering::Relaxed)
    }

    pub fn save_shared(&self, val: T) {
        self.inner_shared.store(val, Ordering::Release);
    }

    pub fn inner(&self) -> &Mutex<U> {
        &self.inner
    }

    pub fn bind(&self, window_id: Option<WindowId>) {
        let mut info = self.info.load(Ordering::Acquire);
        info.window_id = window_id;
        self.info.store(info, Ordering::Release);
    }

    pub fn load_info(&self) -> WindowInfo {
        self.info.load(Ordering::Acquire)
    }

    pub fn detach_to_new_window(&self) {}
}

pub type EmptySubWindowUIState = SubWindowUIState<(), ()>;
impl SubWindowUIState<(), ()> {
    pub fn new_empty(logic_window_id: u64) -> Self {
        Self::new(logic_window_id, (), ())
    }
}

pub struct SubWindow<'open, 'a, U: Sized, T: Sized + Copy> {
    inner: Window<'open>,
    state: &'a SubWindowUIState<U, T>,
    gpu: &'a GpuInstance,
    event_proxy: &'a EventLoopProxy<UserEvent>,
    text: &'a str,
}

impl<'open, 'a, U: Sized, T: Sized + Copy> SubWindow<'open, 'a, U, T> {
    pub fn new(
        gpu: &'a GpuInstance,
        event_proxy: &'a EventLoopProxy<UserEvent>,
        text: &'a str,
        state: &'a SubWindowUIState<U, T>,
    ) -> Self {
        Self {
            inner: Window::new(text),
            state,
            gpu,
            event_proxy,
            text,
        }
    }

    pub fn open(mut self, open: &'open mut bool) -> Self {
        self.inner = self.inner.open(open);
        self
    }

    pub fn show<R>(
        self,
        egui_ctx: &CtxRef,
        add_contents: impl FnOnce(&mut Ui, &mut U, &mut T, &WindowInfo) -> R,
    ) {
        let state = self.state;
        let gpu = self.gpu;
        let mut info = state.load_info();
        let mut render = false;
        if let Some(wid) = info.window_id {
            if wid == gpu.id() {
                render = true;
            }
        } else {
            render = true;
        }
        if render {
            let ret = self.inner.show(egui_ctx, |ui| {
                let mut t = state.load_shared();
                let u = state.inner();
                let mut u = u.lock().unwrap();
                let ret = add_contents(ui, &mut u, &mut t, &mut info);
                state.save_shared(t);
            });
            if let Some(r) = ret {
                if r.response.double_clicked() {
                    self.event_proxy
                        .send_event(UserEvent::Global(GlobalUserEvent::NewWindow(
                            NewWindowProps {
                                title: self.text.to_owned(),
                                size: Size::new(
                                    r.response.rect.width() as u32,
                                    r.response.rect.height() as u32,
                                ),
                                pos: Size::new(
                                    r.response.rect.left() as u32,
                                    r.response.rect.top() as u32,
                                ),
                                drag: false,
                                from_window_id: gpu.id(),
                                logic_window_id: info.logic_window_id,
                            },
                        )));
                }
            }
        }
    }
}
