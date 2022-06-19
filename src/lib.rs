// #![feature(trait_alias)]
// #![feature(concat_idents)]
// mod geometry;
// mod gpu_context;
// mod modules;
// mod render;
// mod render_window;
// mod statistics;
// mod types;
// mod ui;
// mod util;
// mod backend;
// mod event;
// mod looper;

// use std::sync::Arc;

// use gpu_context::{GpuContext, GpuContextRef};

// use render_window::{Queue, RenderWindow, RenderWindowEventLoop};
// use types::Size;
// use ui::logic::UILogic;

// #[cfg(target_arch="wasm32")]
// use wasm_bindgen::prelude::*;

// #[wasm_bindgen]
// #[cfg(target_arch="wasm32")]
// pub fn main() {
//     console_error_panic_hook::set_once();
//     let _ = console_log::init_with_level(log::Level::Trace);

//     let gpu_context: GpuContextRef = GpuContext::new().into();
//     let mut event_loop = RenderWindowEventLoop::new(gpu_context.clone());

//     let size = Size::new(200, 200);
//     let pos = Size::new(0, 0);
//     let mut ui_logic = UILogic::new(gpu_context.clone());
//     let title = "GStudy main".to_owned();

//     event_loop.run(|event_loop, event_proxy, target| {
//         log::info!("event loop start");
//         let (window, resource) = RenderWindow::make_window(title, pos, size, target, &gpu_context);
//         let id = window.id();
//         let queue = Arc::new(Queue::new());
//         event_loop.add_render_window(window, queue.clone());
//         ui_logic.set_main_window_id(id);

//         RenderWindow::new(gpu_context.clone(), queue, id, event_proxy.clone())
//             .dispatch_window(resource, ui_logic, size);
//     });
// }
