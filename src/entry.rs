use crate::backends::WGPUBackend;
use crate::event::EventSource;

pub fn real_main() {
    use crate::{loader::Loader, looper, ui};

    #[cfg(windows)]
    unsafe {
        windows::Win32::System::Console::AttachConsole(u32::MAX);
        windows::Win32::UI::HiDpi::SetProcessDpiAwareness(
            windows::Win32::UI::HiDpi::PROCESS_PER_MONITOR_DPI_AWARE,
        )
        .unwrap();
    }

    let window_builder = winit::window::WindowBuilder::new()
        .with_inner_size(winit::dpi::Size::Logical(winit::dpi::LogicalSize::new(
            1300f64, 900f64,
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

    let loader = Loader::new();
    looper.register_processor(loader.event_processor());

    looper.run();
}
