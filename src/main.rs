#![windows_subsystem = "windows"]
mod canvas;
mod executor;
mod maps;
mod modules;
mod renderer;
mod statistics;
mod types;
mod ui;
mod util;

use canvas::Canvas;
use executor::Executor;
use futures::executor::block_on;
use modules::ModuleInfo;
use renderer::{RenderContext, Renderer, UpdateContext};
use std::time::Instant;
use types::{Color, Size};
use ui::FunctorProvider;
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
    CanvasResize(Size),
    ModuleChanged(&'static str),
}

#[derive(Debug, Clone)]
pub struct FpsState {
    fps_limit: u32,
    enabled: bool,
}
impl Default for FpsState {
    fn default() -> Self {
        Self {
            fps_limit: 60,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UIState {
    show_setting: bool,
    show_inspection: bool,
    show_memory: bool,
    always_repaint: bool,
    first_render: bool,
    fullscreen: bool,
    fps: FpsState,
    update_fps: FpsState,
    clear_color: [f32; 3],
    render_window_size: Size,
    canvas_size: Size,

    modules: Vec<ModuleInfo>,
    select_module: Option<&'static str>,
}
impl Default for UIState {
    fn default() -> Self {
        Self {
            show_setting: false,
            show_inspection: false,
            show_memory: false,
            always_repaint: false,
            first_render: true,
            fullscreen: false,
            fps: FpsState::default(),
            update_fps: FpsState::default(),
            clear_color: [0f32, 0f32, 0f32],
            render_window_size: Size::new(100, 100),
            canvas_size: Size::new(100, 100),

            modules: Vec::new(),
            select_module: None,
        }
    }
}

pub struct FinishUIState {
    cursor: egui::CursorIcon,
}
impl Default for FinishUIState {
    fn default() -> Self {
        Self {
            cursor: egui::CursorIcon::Default,
        }
    }
}

pub struct DrawContext<'b, 'c> {
    state: &'b mut UIState,
    event_proxy: EventLoopProxy<UserEvent>,
    update_context: &'c UpdateContext<'c>,
    ctx: egui::CtxRef,
}

fn draw_fps(
    ui: &mut egui::Ui,
    is_render: bool,
    state: &mut UIState,
    event_proxy: &EventLoopProxy<UserEvent>,
) {
    let first_render = state.first_render;
    let (name, state) = if is_render {
        ("render fps limit", &mut state.fps)
    } else {
        ("update fps limit", &mut state.update_fps)
    };
    ui.label(name);
    ui.horizontal(|ui| {
        let mut changed = ui.checkbox(&mut state.enabled, "").changed();
        ui.scope(|ui| {
            let s = egui::Slider::new(&mut state.fps_limit, 15u32..=250u32).clamp_to_range(true);
            ui.set_enabled(state.enabled);
            if ui.add(s).changed() {
                changed = true;
            }
        });
        if changed || first_render {
            if !state.enabled {
                let _ = event_proxy.send_event(UserEvent::FrameRate((None, is_render)));
                return;
            } else {
                let _ = event_proxy
                    .send_event(UserEvent::FrameRate((Some(state.fps_limit), is_render)));
            }
        }
    });
}

fn control_ui(ui: &mut egui::Ui, context: &mut DrawContext) {
    let DrawContext {
        update_context,
        event_proxy,
        state,
        ctx,
    } = context;

    let UpdateContext {
        render_statistics,
        update_statistics,
    } = update_context;

    ui.label("render fps & fs:");
    ui.label(format!(
        "{:>5.1} {:>6.1}ms",
        render_statistics.fps(),
        render_statistics.frame_secends() * 1000f32
    ));
    ui.end_row();

    ui.label("update fps & fs:");
    ui.label(format!(
        "{:>5.1} {:>6.1}ms",
        update_statistics.fps(),
        update_statistics.frame_secends() * 1000f32
    ));
    ui.end_row();

    ui.label("alaways repaint");
    ui.checkbox(&mut state.always_repaint, "");
    ui.end_row();

    ui.label("full screen");
    if ui.checkbox(&mut state.fullscreen, "").changed() {
        let _ = event_proxy.send_event(UserEvent::FullScreen(state.fullscreen));
    }
    ui.end_row();

    ui.label("settings");
    ui.collapsing("built-in functions", |ui| {
        if ui.button("setting").clicked() {
            state.show_setting = !state.show_setting;
        }
        if ui.button("inspection").clicked() {
            state.show_inspection = !state.show_inspection;
        }
        if ui.button("memory").clicked() {
            state.show_memory = !state.show_memory;
        }
    });
    ui.end_row();

    draw_fps(ui, true, state, &event_proxy);
    ui.end_row();

    draw_fps(ui, false, state, &event_proxy);
    ui.end_row();

    ui.label("clear color");
    if ui.color_edit_button_rgb(&mut state.clear_color).changed() {
        let _ = event_proxy.send_event(UserEvent::ClearColor(Some(Color::new(
            state.clear_color[0],
            state.clear_color[1],
            state.clear_color[2],
            1.0f32,
        ))));
    }
    ui.end_row();

    ui.label("render window width");
    ui.add(egui::Slider::new(
        &mut state.render_window_size.width,
        100..=1024,
    ));
    ui.end_row();

    ui.label("render window height");
    ui.add(egui::Slider::new(
        &mut state.render_window_size.height,
        100..=1024,
    ));
    ui.end_row();

    let mut c = false;
    ui.label("canvas width");
    if ui
        .add(egui::Slider::new(&mut state.canvas_size.width, 100..=1024))
        .changed()
    {
        c = true;
    }
    ui.end_row();

    ui.label("canvas height");
    if ui
        .add(egui::Slider::new(&mut state.canvas_size.height, 100..=1024))
        .changed()
    {
        c = true;
    }
    if c {
        let _ = event_proxy.send_event(UserEvent::CanvasResize(state.canvas_size));
    }

    ui.end_row();

    if render_statistics.changed() || update_statistics.changed() {
        ctx.request_repaint();
    }
}


fn module_item(ui: &mut egui::Ui, info: &ModuleInfo, select: &mut Option<&'static str>, mut rect: egui::Rect,
               beg: bool, event_proxy: &mut EventLoopProxy<UserEvent>) -> f32 {

    // init position
    let fonts = ui.fonts();
    let row_height0 = fonts[egui::TextStyle::Button].row_height();
    let row_height1 = fonts[egui::TextStyle::Body].row_height();
    let space = ui.spacing().item_spacing.y;
    let line_stroke = ui.visuals().widgets.noninteractive.bg_stroke;

    let pos0 = rect.left_top() + ui.spacing().item_spacing;
    let pos1 = egui::pos2(pos0.x, pos0.y + row_height0 + space);
    let inner_height = row_height0 + row_height1 + space * 3f32;
    let height = inner_height + line_stroke.width;
    rect.set_height(height);

    let mut inner_rect = rect;
    inner_rect.set_top(rect.top() + line_stroke.width);
    inner_rect.set_height(inner_height - line_stroke.width * 2f32);

    // allocate ui content rectangle
    let r = ui.allocate_rect(inner_rect, egui::Sense::click());
    let mut has_outer_box = false;
    let widget =
        loop {
            if r.hovered() {
                has_outer_box = true;
                break ui.visuals().widgets.hovered;
            } else {
                if let Some(v) = select {
                    if *v == info.name {
                        has_outer_box = true;
                        break ui.visuals().widgets.active;
                    }
                }
                break ui.visuals().widgets.inactive;
            }
        };


    // render
    let painter = ui.painter();
    if has_outer_box {
        painter.rect(inner_rect, widget.corner_radius, widget.bg_fill, widget.bg_stroke);
    }

    let color0 = widget.fg_stroke.color;
    let color1 = egui::color::tint_color_towards(color0, ui.visuals().window_fill());

    painter.text(pos0, egui::Align2::LEFT_TOP, info.name, egui::TextStyle::Button, color0);
    painter.text(pos1, egui::Align2::LEFT_TOP, info.desc, egui::TextStyle::Body, color1);

    if !beg {
        // fill segment
        let vec = egui::vec2(0f32, line_stroke.width);
        painter.line_segment([rect.left_top() - vec, rect.right_top() - vec], line_stroke);
    }


    if r.double_clicked() {
        *select = Some(info.name);
        let _ = event_proxy.send_event(UserEvent::ModuleChanged(info.name));
    }
    height
}

fn modules_ui(ui: &mut egui::Ui, context: &mut DrawContext) {
    let DrawContext {
        update_context,
        event_proxy,
        state,
        ctx,
    } = context;
    egui::ScrollArea::auto_sized().show_viewport(ui,|ui, viewport| {
        let mut height = 0f32;
        let mut top = viewport.min.y + ui.max_rect().top();
        let left = ui.max_rect().left();
        let width = ui.max_rect().width();

        let mut beg = true;
        for item in &state.modules {
            let rect = egui::Rect::from_min_size(egui::pos2(left, top as f32),
            egui::vec2(width, height));
            height = module_item(ui, item, &mut state.select_module, rect, beg, event_proxy);
            top += height;
            beg = false;
        }
    });
}

fn draw(
    ctx: egui::CtxRef,
    update_context: &UpdateContext,
    state: &mut UIState,
    event_proxy: EventLoopProxy<UserEvent>,
) {
    let mut context = DrawContext {
        ctx: ctx.clone(),
        update_context,
        state,
        event_proxy: event_proxy.clone(),
    };

    egui::Window::new("Control")
        .resizable(true)
        .scroll(true)
        .show(&ctx, |ui| {
            egui::Grid::new("grid")
                .striped(true)
                .spacing([40.0, 4.0])
                .show(ui, |ui| control_ui(ui, &mut context));
        });

    egui::Window::new("Modules")
        .resizable(true)
        .scroll(true)
        .show(&ctx, |ui| {
            modules_ui(ui, &mut context);
        });

    std::mem::drop(context);

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

    egui::Window::new("Memory")
        .resizable(true)
        .open(&mut state.show_memory)
        .scroll(true)
        .show(&ctx, |ui| {
            ctx.memory_ui(ui);
        });

    egui::Window::new("Render window")
        .resizable(false)
        .min_width(1f32)
        .min_height(1f32)
        .collapsible(false)
        .show(&ctx, |ui| {
            let size = egui::vec2(
                state.render_window_size.width as f32,
                state.render_window_size.height as f32,
            );
            ui.image(egui::TextureId::User(0), size);
        })
        .unwrap();

    state.first_render = false;
    if state.always_repaint {
        ctx.request_repaint();
    }
}

fn finish_ui(
    output: &egui::Output,
    state: &mut FinishUIState,
    event_proxy: EventLoopProxy<UserEvent>,
) {
    if output.cursor_icon != state.cursor {
        if output.cursor_icon == egui::CursorIcon::None {
            let _ = event_proxy.send_event(UserEvent::UpdateCursor(None));
        } else {
            let _ = event_proxy.send_event(UserEvent::UpdateCursor(Some(
                maps::match_winit_cursor(output.cursor_icon),
            )));
        }
        state.cursor = output.cursor_icon;
    }
    if let Some(url) = &output.open_url {
        log::info!("open url {}", url.url);
        tinyfiledialogs::message_box_ok(
            "open url",
            &url.url,
            tinyfiledialogs::MessageBoxIcon::Info,
        );
    }
    if let Some(pos) = &output.text_cursor_pos {
        let _ = event_proxy.send_event(UserEvent::UpdateIme(
            LogicalPosition::new(pos.x, pos.y).into(),
        ));
    }
    if !output.copied_text.is_empty() {
        log::info!("copy {}", output.copied_text);
        tinyfiledialogs::message_box_ok(
            "copy string",
            &output.copied_text,
            tinyfiledialogs::MessageBoxIcon::Info,
        );
    }
}

pub struct UIContext {
    canvas: Canvas,
    state: UIState,
    finish_state: FinishUIState,
    event_proxy: EventLoopProxy<UserEvent>,
    executor: Executor,
}

impl UIContext {
    pub fn new(canvas: Canvas, event_proxy: EventLoopProxy<UserEvent>) -> Self {
        let executor = Executor::new();
        let mut res = Self {
            canvas,
            event_proxy,
            state: UIState::default(),
            finish_state: FinishUIState::default(),
            executor,
        };
        res.state.modules = res.executor.list();
        res
    }
}

impl FunctorProvider for UIContext {
    fn prepare_texture(&mut self, ctx: RenderContext) {
        self.canvas.build_texture(ctx);
    }

    fn get_texture<'s>(&'s self, id: u64) -> &'s wgpu::BindGroup {
        match id {
            0 => self.canvas.get_texture().1,
            _ => {
                panic!("invalid texture id");
            }
        }
    }

    fn update(&mut self, ctx: egui::CtxRef, update_context: &UpdateContext) {
        draw(
            ctx,
            update_context,
            &mut self.state,
            self.event_proxy.clone(),
        );
    }

    fn finish(&mut self, output: &egui::Output) {
        finish_ui(output, &mut self.finish_state, self.event_proxy.clone());
    }

    fn on_user_event(&mut self, event: &UserEvent) {
        match event {
            &UserEvent::CanvasResize(size) => {
                self.canvas.resize_pixels(size);
            }
            &UserEvent::ModuleChanged(name) => {
                self.executor.run(name);
            }
            _ => (),
        }
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

    let canvas = canvas::Canvas::new(Size::new(100, 100));
    let provider = UIContext::new(canvas, event_proxy.clone());

    let ui = Box::new(ui::UI::new(provider));

    let mut renderer = block_on(Renderer::new(&window));
    renderer.add(ui);

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
