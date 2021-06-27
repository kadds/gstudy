mod canvas;
mod maps;
mod renderer;
mod statistics;
mod types;
mod ui;
mod util;

use futures::executor::block_on;
use renderer::{Renderer, UpdateContext};
use std::time::Instant;
use types::{Color, Rect, Size};
use winit::{
    dpi::{LogicalPosition, LogicalSize, Position},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopProxy},
    platform::run_return::EventLoopExtRunReturn,
    window::{CursorIcon, WindowBuilder},
};

#[derive(Debug, Clone)]
pub enum UserEvent {
    FrameRate((Option<u32>, bool)),
    UpdateCursor(Option<CursorIcon>),
    UpdateIme(Position),
    ClearColor(Option<Color>),
    FullScreen(bool),
    MoveCanvas(Rect),
}

#[derive(Debug, Clone)]
pub struct FpsState {
    fps_limit: String,
    fps_error: String,
}

impl Default for FpsState {
    fn default() -> Self {
        Self {
            fps_limit: "60".to_owned(),
            fps_error: "".to_owned(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UIState {
    show_setting: bool,
    show_inspection: bool,
    always_repaint: bool,
    first_render: bool,
    fullscreen: bool,
    fps: FpsState,
    update_fps: FpsState,
    clear_color: [f32; 3],
    last_render_window_rect: egui::Rect,
    render_window_size: egui::Vec2,
}
impl Default for UIState {
    fn default() -> Self {
        Self {
            show_setting: false,
            show_inspection: false,
            always_repaint: false,
            first_render: true,
            fullscreen: false,
            fps: FpsState::default(),
            update_fps: FpsState::default(),
            clear_color: [0f32, 0f32, 0f32],
            last_render_window_rect: egui::Rect::from_min_max(
                egui::Pos2::new(0f32, 0f32),
                egui::Pos2::new(0f32, 0f32),
            ),
            render_window_size: egui::vec2(100f32, 100f32),
        }
    }
}

fn draw_fps(
    ui: &mut egui::Ui,
    first_render: bool,
    state: &mut FpsState,
    is_render: bool,
    proxy: &EventLoopProxy<UserEvent>,
) {
    ui.horizontal(|ui| {
        let name = if is_render {
            "render fps limit"
        } else {
            "update fps limit"
        };
        ui.label(name);
        let fps_ui = ui.text_edit_singleline(&mut state.fps_limit);
        if fps_ui.lost_focus() || first_render {
            if state.fps_limit.is_empty() {
                let _ = proxy.send_event(UserEvent::FrameRate((None, is_render)));
                state.fps_error = "".to_owned();
                return;
            }
            match state.fps_limit.parse::<u32>() {
                Ok(v) => {
                    let _ = proxy.send_event(UserEvent::FrameRate((Some(v as u32), is_render)));
                    state.fps_error = "".to_owned();
                }
                Err(_) => {
                    state.fps_error = format!("error");
                }
            };
        }
        if !state.fps_error.is_empty() {
            ui.colored_label(egui::Color32::RED, &state.fps_error);
        }
    });
}

fn draw_ui(
    ctx: egui::CtxRef,
    update_ctx: &UpdateContext,
    state: &mut UIState,
    proxy: EventLoopProxy<UserEvent>,
) {
    egui::Window::new("Control")
        .resizable(true)
        .scroll(true)
        .show(&ctx, |ui| {
            ui.label(format!(
                "render fps:{:.1} fs:{:.1}ms",
                update_ctx.render_statistics.fps(),
                update_ctx.render_statistics.frame_secends() * 1000f32
            ));
            ui.label(format!(
                "update fps:{:.1} fs:{:.1}ms",
                update_ctx.update_statistics.fps(),
                update_ctx.update_statistics.frame_secends() * 1000f32
            ));
            ui.checkbox(&mut state.always_repaint, "always repaint");
            if ui.checkbox(&mut state.fullscreen, "fullscreen").changed() {
                let _ = proxy.send_event(UserEvent::FullScreen(state.fullscreen));
            }
            if ui.button("setting").clicked() {
                state.show_setting = !state.show_setting;
            }
            if ui.button("inspection").clicked() {
                state.show_inspection = !state.show_inspection;
            }
            draw_fps(ui, state.first_render, &mut state.fps, true, &proxy);
            draw_fps(ui, state.first_render, &mut state.update_fps, false, &proxy);
            ui.vertical(|ui| {
                ui.label("clear color");
                if ui.color_edit_button_rgb(&mut state.clear_color).changed() {
                    let _ = proxy.send_event(UserEvent::ClearColor(Some(Color::new(
                        state.clear_color[0],
                        state.clear_color[1],
                        state.clear_color[2],
                        1.0f32,
                    ))));
                }
            });
        });
    if state.always_repaint {
        ctx.request_repaint();
    }
    egui::Window::new("Setting")
        .resizable(true)
        .open(&mut state.show_setting)
        .scroll(true)
        .show(&ctx, |ui| {
            ctx.settings_ui(ui);
        });
    egui::Window::new("Inspection")
        .resizable(true)
        .open(&mut state.show_inspection)
        .scroll(true)
        .show(&ctx, |ui| {
            ctx.inspection_ui(ui);
        });

    egui::Window::new("Render window")
        .fixed_size(egui::vec2(100f32, 100f32))
        .resizable(true)
        .collapsible(false)
        .show(&ctx, |ui| {
            let (_, r) = ui.allocate_exact_size(
                state.render_window_size,
                egui::Sense::focusable_noninteractive(),
            );
            if r.rect != state.last_render_window_rect {
                state.last_render_window_rect = r.rect;
                let rect = r.rect;
                let _ = proxy.send_event(UserEvent::MoveCanvas(Rect::new(
                    rect.left() as u32,
                    rect.top() as u32,
                    rect.width() as u32,
                    rect.height() as u32,
                )));
            }
        })
        .unwrap();

    state.first_render = false;
}

fn set_ui(output: &egui::Output, proxy: EventLoopProxy<UserEvent>) {
    if output.cursor_icon == egui::CursorIcon::None {
        let _ = proxy.send_event(UserEvent::UpdateCursor(None));
    } else {
        let _ = proxy.send_event(UserEvent::UpdateCursor(Some(maps::match_winit_cursor(
            output.cursor_icon,
        ))));
    }
    if let Some(url) = &output.open_url {
        log::info!("open url {}", url.url);
    }
    if let Some(pos) = &output.text_cursor_pos {
        let _ = proxy.send_event(UserEvent::UpdateIme(
            LogicalPosition::new(pos.x, pos.y).into(),
        ));
    }
    if !output.copied_text.is_empty() {
        log::info!("copy {}", output.copied_text);
    }
}

fn main() {
    env_logger::init();
    let mut event_loop = EventLoop::with_user_event();
    let event_proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_inner_size(LogicalSize::new(800, 600))
        .with_title("GStudy")
        .with_visible(true)
        .build(&event_loop)
        .unwrap();

    let temp_event_proxy = event_proxy.clone();
    let mut ui_state = UIState::default();
    let ui = Box::new(ui::UI::new(
        Box::new(move |c, u| draw_ui(c, u, &mut ui_state, temp_event_proxy.clone())),
        Box::new(move |v| set_ui(v, event_proxy.clone())),
    ));
    let canvas = Box::new(canvas::Canvas::new(Size::new(100, 100)));
    let mut renderer = block_on(Renderer::new(&window));
    renderer.add(ui);
    renderer.add(canvas);

    let mut target_tick = Instant::now();
    let mut update_tick = target_tick;
    let mut render_tick = target_tick;
    let mut not_render = false;
    let mut need_redraw = true;

    event_loop.run_return(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(target_tick);
        match event {
            Event::WindowEvent {
                event,
                window_id: _,
            } => {
                renderer.on_event(&event);
                match event {
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                    _ => (),
                };
            }
            Event::MainEventsCleared => {
                let now = Instant::now();
                if now >= target_tick {
                    window.request_redraw();
                    *control_flow = ControlFlow::Wait; // the next tick
                    need_redraw = true;
                }
            }
            Event::RedrawRequested(_) => {
                if !need_redraw {
                    log::trace!("redraw skip");
                    return;
                }
                if update_tick == target_tick {
                    let (next_tick, need_render) = renderer.update();
                    not_render = !need_render;
                    update_tick = next_tick;
                } else {
                    render_tick = renderer.render()
                };
                if not_render {
                    target_tick = update_tick;
                } else {
                    target_tick = update_tick.min(render_tick);
                }
                need_redraw = false;
                // log::info!("{:?} u {:?} t {:?}", render_tick, update_tick, target_tick);
                *control_flow = ControlFlow::WaitUntil(target_tick);
            }
            Event::UserEvent(e) => {
                renderer.on_user_event(&e);
                match e {
                    UserEvent::FrameRate((rate, is_render)) => {
                        if is_render {
                            renderer.set_frame_lock(rate.map(|v| 1f32 / v as f32));
                        } else {
                            renderer.set_update_frame_lock(rate.map(|v| 1f32 / v as f32));
                        }
                    }
                    UserEvent::UpdateCursor(cursor) => match cursor {
                        Some(c) => {
                            window.set_cursor_visible(true);
                            window.set_cursor_icon(c);
                        }
                        None => {
                            window.set_cursor_visible(false);
                        }
                    },
                    UserEvent::UpdateIme(pos) => {
                        window.set_ime_position(pos);
                    }
                    UserEvent::ClearColor(c) => {
                        renderer.set_clear_color(c.map(|c| wgpu::Color {
                            r: c.r as f64,
                            b: c.b as f64,
                            g: c.g as f64,
                            a: c.a as f64,
                        }));
                    }
                    UserEvent::FullScreen(set) => {
                        if set {
                            window
                                .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                        } else {
                            window.set_fullscreen(None);
                        }
                    }
                    _ => (),
                }
            }
            _ => (),
        }
    });
}
