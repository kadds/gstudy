use crate::loader::ResourceManager;
use core::backends::WGPUBackend;
use std::sync::Arc;

#[allow(unused)]
pub fn real_main() {
    use crate::{loader::Loader, looper};

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

    let backend = Box::new(WGPUBackend::new(looper.window()).unwrap());

    looper.register_processor(backend.event_processor());
    let gpu = backend.gpu();
    let resource_manager = Arc::new(ResourceManager::new(gpu));
    looper.bind(backend, resource_manager.clone());

    let loader = Loader::new(resource_manager);
    looper.register_processor(loader.event_processor());

    looper.run();
}
