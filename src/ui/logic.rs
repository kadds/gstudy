use std::sync::{
    atomic::{AtomicPtr, Ordering},
    Arc, Mutex,
};

use winit::{dpi::LogicalPosition, event_loop::EventLoopProxy, window::WindowId};

use crate::{
    gpu_context::{GpuContext, GpuContextRef},
    modules::ModuleInfo,
    render::{Canvas, Executor},
    render_window::{GlobalUserEvent, UserEvent, WindowUserEvent},
    statistics::Statistics,
    types::{Vec2f, Vec4f},
    util::match_winit_cursor,
};

use super::subwindow::*;

type Size = crate::types::Size;

pub struct UIState {
    control: SubWindowUIState<ControlUIState, ControlUISharedState>,
    setting: EmptySubWindowUIState,
    inspection: EmptySubWindowUIState,
    memory: EmptySubWindowUIState,
    render_window: EmptySubWindowUIState,
    module: SubWindowUIState<ModuleUIState, ()>,
}

pub struct UILogic {
    canvas: Arc<Canvas>,
    ui_state: UIState,
    // executor: Executor,
    gpu_context: GpuContextRef,
}

pub type UILogicRef = Arc<UILogic>;

impl UILogic {
    pub fn new(gpu_context: GpuContextRef) -> Self {
        let canvas = Arc::new(Canvas::new(Size::new(400, 400)));
        let executor = Executor::new(gpu_context.clone());
        let res = Self {
            canvas: canvas,
            ui_state: UIState {
                control: SubWindowUIState::new(
                    1,
                    ControlUIState::default(),
                    ControlUISharedState::default(),
                ),
                setting: EmptySubWindowUIState::new_empty(2),
                inspection: EmptySubWindowUIState::new_empty(3),
                memory: EmptySubWindowUIState::new_empty(4),
                render_window: EmptySubWindowUIState::new_empty(5),
                module: SubWindowUIState::new(
                    6,
                    ModuleUIState {
                        modules: executor.list(),
                        select_module: None,
                    },
                    (),
                ),
            },
            // executor,
            gpu_context,
        };
        res
    }

    pub fn rebind_logic_window(&self, logic_window_id: u64) {
        let id = Some(self.gpu_context.instance().id());
        if logic_window_id == 0 {
            self.ui_state.control.bind(id);
            self.ui_state.setting.bind(id);
            self.ui_state.inspection.bind(id);
            self.ui_state.memory.bind(id);
            self.ui_state.render_window.bind(id);
            self.ui_state.module.bind(id);
        } else {
            match logic_window_id {
                1 => self.ui_state.control.bind(id),
                2 => self.ui_state.setting.bind(id),
                3 => self.ui_state.inspection.bind(id),
                4 => self.ui_state.memory.bind(id),
                5 => self.ui_state.render_window.bind(id),
                6 => self.ui_state.module.bind(id),
                _ => {
                    panic!("invalid logic_window_id")
                }
            }
        }
    }

    pub fn prepare_texture(&self) {
        self.canvas.build_texture(&self.gpu_context.instance());
    }

    pub fn update(
        &self,
        ctx: egui::CtxRef,
        statistics: &Statistics,
        event_proxy: &EventLoopProxy<UserEvent>,
    ) {
        let gpu = self.gpu_context.instance();
        SubWindow::new(&gpu, &event_proxy, "Control", &self.ui_state.control).show(
            &ctx,
            |ui, state, shared_state, _| {
                egui::Grid::new("grid")
                    .striped(true)
                    .spacing([40.0, 4.0])
                    .show(ui, |ui| {
                        control_ui(gpu.id(), ui, state, shared_state, statistics, event_proxy)
                    });
            },
        );

        SubWindow::new(&gpu, event_proxy, "Modules", &self.ui_state.module).show(
            &ctx,
            |ui, state, _, _| {
                modules_ui(ui, state, event_proxy);
            },
        );

        let mut state = self.ui_state.control.load_shared();
        SubWindow::new(&gpu, event_proxy, "Setting", &self.ui_state.setting)
            .open(&mut state.show_setting)
            .show(&ctx, |ui, _, _, _| {
                ctx.settings_ui(ui);
            });
        SubWindow::new(&gpu, event_proxy, "Inspection", &self.ui_state.inspection)
            .open(&mut state.show_inspection)
            .show(&ctx, |ui, _, _, _| {
                ctx.inspection_ui(ui);
            });
        SubWindow::new(&gpu, event_proxy, "Memory", &self.ui_state.memory)
            .open(&mut state.show_memory)
            .show(&ctx, |ui, _, _, _| {
                ctx.memory_ui(ui);
            });
        self.ui_state.control.save_shared(state);

        SubWindow::new(
            &gpu,
            event_proxy,
            "Render window",
            &self.ui_state.render_window,
        )
        .show(&ctx, |ui, state, _, g| {
            ui.image(
                egui::TextureId::User(0),
                egui::vec2(g.size.x as f32, g.size.y as f32),
            );
        });

        // state.first_render = false;
        if self.ui_state.control.inner().lock().unwrap().always_repaint {
            ctx.request_repaint();
        }
    }

    pub fn finish(
        &self,
        output: &egui::Output,
        cursor: egui::CursorIcon,
        event_proxy: &EventLoopProxy<UserEvent>,
    ) {
        let id = self.gpu_context.instance().id();
        {
            if output.cursor_icon != cursor {
                let _ = event_proxy.send_event(UserEvent::Window(
                    id,
                    WindowUserEvent::UpdateCursor(output.cursor_icon),
                ));
            }
        }
        if let Some(url) = &output.open_url {
            let _ = event_proxy.send_event(UserEvent::Global(GlobalUserEvent::OpenUrl(
                url.url.to_owned(),
            )));
        }
        if let Some(pos) = &output.text_cursor_pos {
            let _ = event_proxy.send_event(UserEvent::Window(
                id,
                WindowUserEvent::UpdateIme(LogicalPosition::new(pos.x, pos.y).into()),
            ));
        }
        if !output.copied_text.is_empty() {
            log::info!("copy {}", output.copied_text);
            let _ = event_proxy.send_event(UserEvent::Global(GlobalUserEvent::Copy(
                output.copied_text.to_owned(),
            )));
        }
    }

    pub fn on_user_event(&self, event: &UserEvent) {
        match event {
            UserEvent::Global(global) => match global {
                GlobalUserEvent::CanvasResize(size) => {
                    // self.canvas = Arc::new(Canvas::new(size));
                    // self.executor.rerun(self.canvas.clone());
                }
                GlobalUserEvent::ModuleChanged(name) => {
                    // self.executor.run(name, self.canvas.clone());
                }
                _ => (),
            },
            _ => (),
        }
    }

    pub fn get_texture<'s>(&'s self, id: u64) -> &'s wgpu::BindGroup {
        match id {
            0 => self.canvas.get_texture().2,
            _ => {
                panic!("invalid texture id");
            }
        }
    }
}

#[derive(Debug, Copy, Clone)]
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
#[derive(Debug, Copy, Clone)]

pub struct ControlUISharedState {
    show_setting: bool,
    show_inspection: bool,
    show_memory: bool,
    canvas_size: Option<Size>,
}

#[derive(Debug, Copy, Clone)]
pub struct ControlUIState {
    always_repaint: bool,
    first_render: bool,
    fullscreen: bool,
    clear_color: [f32; 3],
    fps: FpsState,
}

impl Default for ControlUIState {
    fn default() -> Self {
        Self {
            always_repaint: false,
            first_render: true,
            fullscreen: false,
            clear_color: [0f32, 0f32, 0f32],
            fps: FpsState::default(),
        }
    }
}

impl Default for ControlUISharedState {
    fn default() -> Self {
        Self {
            show_setting: false,
            show_inspection: false,
            show_memory: false,
            canvas_size: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModuleUIState {
    modules: Vec<ModuleInfo>,
    select_module: Option<&'static str>,
}

impl Default for ModuleUIState {
    fn default() -> Self {
        Self {
            modules: Vec::new(),
            select_module: None,
        }
    }
}

fn draw_fps(
    id: WindowId,
    ui: &mut egui::Ui,
    state: &mut ControlUIState,
    event_proxy: &EventLoopProxy<UserEvent>,
) {
    let first_render = state.first_render;
    let name = "render_fps_limit";
    let state = &mut state.fps;
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
        if changed {
            if !state.enabled {
                event_proxy
                    .send_event(UserEvent::Window(id, WindowUserEvent::FrameRate(None)))
                    .unwrap();
            } else {
                event_proxy
                    .send_event(UserEvent::Window(
                        id,
                        WindowUserEvent::FrameRate(Some(state.fps_limit)),
                    ))
                    .unwrap();
            }
        }
    });
}

fn control_ui(
    id: WindowId,
    ui: &mut egui::Ui,
    state: &mut ControlUIState,
    shared_state: &mut ControlUISharedState,
    statistics: &Statistics,
    event_proxy: &EventLoopProxy<UserEvent>,
) {
    ui.label("fps & fs:");
    ui.label(format!(
        "{:>5.1} {:>6.1}ms",
        statistics.fps(),
        statistics.frame_secends() * 1000f32
    ));
    ui.end_row();

    ui.label("alaways repaint");
    ui.checkbox(&mut state.always_repaint, "");
    ui.end_row();

    ui.label("full screen");
    if ui.checkbox(&mut state.fullscreen, "").changed() {
        // let _ = event_proxy.send_event(UserEvent::FullScreen(state.fullscreen));
    }
    ui.end_row();

    ui.label("settings");
    ui.collapsing("built-in functions", |ui| {
        if ui.button("setting").clicked() {
            shared_state.show_setting = !shared_state.show_setting;
        }
        if ui.button("inspection").clicked() {
            shared_state.show_inspection = !shared_state.show_inspection;
        }
        if ui.button("memory").clicked() {
            shared_state.show_memory = !shared_state.show_memory;
        }
    });
    ui.end_row();

    draw_fps(id, ui, state, &event_proxy);
    ui.end_row();

    ui.label("clear color");
    if ui.color_edit_button_rgb(&mut state.clear_color).changed() {
        // let _ = event_proxy.send_event(UserEvent::ClearColor(Some(Vec4f::new(
        //     state.clear_color[0],
        //     state.clear_color[1],
        //     state.clear_color[2],
        //     1.0f32,
        // ))));
    }
    ui.end_row();

    // ui.label("render window width");
    // ui.add(egui::Slider::new(
    //     &mut state.render_window_size.x,
    //     100..=1024,
    // ));
    // ui.end_row();

    // ui.label("render window height");
    // ui.add(egui::Slider::new(
    //     &mut state.render_window_size.y,
    //     100..=1024,
    // ));
    // ui.end_row();

    // let mut c = false;
    // ui.label("canvas width");
    // if ui
    //     .add(egui::Slider::new(&mut state.canvas_size.x, 100..=1024))
    //     .changed()
    // {
    //     c = true;
    // }
    // ui.end_row();

    // ui.label("canvas height");
    // if ui
    //     .add(egui::Slider::new(&mut state.canvas_size.y, 100..=1024))
    //     .changed()
    // {
    //     c = true;
    // }
    // if c {
    //     let _ = event_proxy.send_event(UserEvent::CanvasResize(state.canvas_size));
    // }

    ui.end_row();

    if statistics.changed() {
        ui.ctx().request_repaint();
    }
}

fn module_item(
    ui: &mut egui::Ui,
    info: &ModuleInfo,
    select: &mut Option<&'static str>,
    mut rect: egui::Rect,
    beg: bool,
    event_proxy: &EventLoopProxy<UserEvent>,
) -> f32 {
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
    let widget = loop {
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
        painter.rect(
            inner_rect,
            widget.corner_radius,
            widget.bg_fill,
            widget.bg_stroke,
        );
    }

    let color0 = widget.fg_stroke.color;
    let color1 = egui::color::tint_color_towards(color0, ui.visuals().window_fill());

    painter.text(
        pos0,
        egui::Align2::LEFT_TOP,
        info.name,
        egui::TextStyle::Button,
        color0,
    );
    painter.text(
        pos1,
        egui::Align2::LEFT_TOP,
        info.desc,
        egui::TextStyle::Body,
        color1,
    );

    if !beg {
        // fill segment
        let vec = egui::vec2(0f32, line_stroke.width);
        painter.line_segment([rect.left_top() - vec, rect.right_top() - vec], line_stroke);
    }

    if r.double_clicked() {
        *select = Some(info.name);
        // let _ = event_proxy.send_event(UserEvent::ModuleChanged(info.name));
    }
    height
}

fn modules_ui(
    ui: &mut egui::Ui,
    state: &mut ModuleUIState,
    event_proxy: &EventLoopProxy<UserEvent>,
) {
    egui::ScrollArea::auto_sized().show_viewport(ui, |ui, viewport| {
        let mut height = 0f32;
        let mut top = viewport.min.y + ui.max_rect().top();
        let left = ui.max_rect().left();
        let width = ui.max_rect().width();

        let mut beg = true;
        for item in &state.modules {
            let rect =
                egui::Rect::from_min_size(egui::pos2(left, top as f32), egui::vec2(width, height));
            height = module_item(ui, item, &mut state.select_module, rect, beg, event_proxy);
            top += height;
            beg = false;
        }
    });
}
