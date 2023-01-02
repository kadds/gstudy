#![feature(trait_upcasting)]
#![feature(strict_provenance)]
#![windows_subsystem = "windows"]
mod core;
mod entry;
mod event;
mod geometry;
mod loader;
mod looper;
mod main_loop;
mod model;
mod modules;
mod render;
mod statistics;
mod types;
mod ui;
mod util;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    env_logger::init();
    entry::real_main();
}
