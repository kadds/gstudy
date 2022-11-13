mod backends;
mod event;
mod geometry;
mod looper;
mod modules;
mod render;
mod statistics;
mod types;
mod ui;
mod util;
use backends::WGPUBackend;
use event::EventSource;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[cfg(target_arch = "wasm32")]
pub fn main() {
    console_log::init_with_level(log::Level::Info);
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    let window_builder = winit::window::WindowBuilder::new()
        .with_inner_size(winit::dpi::Size::Logical(winit::dpi::LogicalSize::new(
            1920f64, 1080f64,
        )))
        .with_resizable(true)
        .with_visible(false)
        .with_title("GStudy");

    let mut looper = looper::Looper::new(window_builder);
    looper.register_processor(Box::new(looper::DefaultProcessor::new()));

    let ui = ui::UI::new();
    looper.register_processor(ui.event_processor());

    let backend = WGPUBackend::new(looper.window()).unwrap();
    looper.register_processor(backend.event_processor());
    looper.bind_backend(backend);

    looper.run();

    // console_error_panic_hook::set_once();
    // let _ = console_log::init_with_level(log::Level::Trace);

    // let gpu_context: GpuContextRef = GpuContext::new().into();
    // let mut event_loop = RenderWindowEventLoop::new(gpu_context.clone());

    // let size = Size::new(200, 200);
    // let pos = Size::new(0, 0);
    // let mut ui_logic = UILogic::new(gpu_context.clone());
    // let title = "GStudy main".to_owned();

    // event_loop.run(|event_loop, event_proxy, target| {
    //     log::info!("event loop start");
    //     let (window, resource) = RenderWindow::make_window(title, pos, size, target, &gpu_context);
    //     let id = window.id();
    //     let queue = Arc::new(Queue::new());
    //     event_loop.add_render_window(window, queue.clone());
    //     ui_logic.set_main_window_id(id);

    //     RenderWindow::new(gpu_context.clone(), queue, id, event_proxy.clone())
    //         .dispatch_window(resource, ui_logic, size);
    // });
}
