#![feature(trait_alias)]
#![feature(concat_idents)]
#![feature(thread_spawn_unchecked)]
#![windows_subsystem = "windows"]
mod geometry;
mod gpu_context;
mod modules;
mod render;
mod render_window;
mod statistics;
mod types;
mod ui;
mod util;

use std::sync::Arc;

use gpu_context::{GpuContext, GpuContextRef};

use render_window::{Queue, RenderWindow, RenderWindowEventLoop};
use types::{Color, Size};
use ui::logic::UILogic;

fn main() {
    #[cfg(windows)]
    unsafe {
        windows::Win32::System::Console::AttachConsole(u32::MAX);
    }
    env_logger::init();
    let gpu_context: GpuContextRef = GpuContext::new().into();
    let mut event_loop = RenderWindowEventLoop::new(gpu_context.clone());

    let size = Size::new(800, 600);
    let pos = Size::new(0, 0);
    let ui_logic = UILogic::new(gpu_context.clone()).into();
    let title = "GStudy main".to_owned();

    event_loop.run(|event_loop, event_proxy, target| {
        let (window, resource) = RenderWindow::make_window(title, pos, size, target, &gpu_context);
        let id = window.id();
        let queue = Arc::new(Queue::new());
        event_loop.add_render_window(window, queue.clone());

        RenderWindow::new(gpu_context.clone(), queue, id, event_proxy.clone())
            .dispatch_window(0, resource, ui_logic, size);
    });
}
