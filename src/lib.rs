mod backends;
mod entry;
mod event;
mod geometry;
mod loader;
mod looper;
mod model;
mod modules;
mod render;
mod statistics;
mod types;
mod ui;
mod util;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[cfg(target_arch = "wasm32")]
pub fn main() {
    console_log::init_with_level(log::Level::Info);
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    entry::real_main();
}
