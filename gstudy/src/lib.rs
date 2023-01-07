#![feature(trait_upcasting)]
#![feature(strict_provenance)]
mod entry;
mod loader;
mod logic;
mod looper;
mod statistics;
mod taskpool;
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
