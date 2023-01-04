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
    use slog::Drain;

    let decorator = slog_term::PlainSyncDecorator::new(std::io::stdout());
    let drain = slog_term::FullFormat::new(decorator).build().fuse();

    let drain = slog_async::Async::new(drain).build().fuse();

    let _g = slog_envlogger::new(drain);

    let _log = slog::Logger::root(_g, slog::o!());
    let _scope_guard = slog_scope::set_global_logger(_log);
    let _log_guard = slog_stdlog::init().unwrap();

    entry::real_main();
}
