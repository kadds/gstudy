// #![feature(trait_alias)]
// #![feature(concat_idents)]
// #![feature(thread_spawn_unchecked)]
#![windows_subsystem = "windows"]
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

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    #[cfg(windows)]
    unsafe {
        windows::Win32::System::Console::AttachConsole(u32::MAX);
    }
    env_logger::init();

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
}
