// #![feature(trait_alias)]
// #![feature(concat_idents)]
// #![feature(thread_spawn_unchecked)]
#![windows_subsystem = "windows"]
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

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    env_logger::init();
    entry::real_main();
}
