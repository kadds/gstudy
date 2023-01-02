use std::sync::Arc;

use crate::core::backends::WGPUBackend;
use crate::event::EventSource;
use crate::loader::ResourceManager;
use crate::main_loop;

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
    looper.register_processor(Box::new(looper::DefaultProcessor::new()));

    let backend = Box::new(WGPUBackend::new(looper.window()).unwrap());
    let loopx = main_loop::MainLoop::new(backend.gpu());

    for ev in loopx.internal_processors() {
        looper.register_processor(ev);
    }

    looper.register_processor(backend.event_processor());
    let gpu = backend.gpu();
    looper.bind_backend(backend);

    let resource_manager = Arc::new(ResourceManager::new(gpu));
    looper.bind_resource_manager(resource_manager.clone());

    let loader = Loader::new(resource_manager.clone());
    looper.register_processor(loader.event_processor());

    looper.run();
}
