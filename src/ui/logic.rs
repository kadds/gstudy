use std::sync::{Arc, Mutex};

use winit::{dpi::LogicalPosition, event_loop::EventLoopProxy, window::WindowId};

use crate::{
    gpu_context::GpuContextRef,
    modules::ModuleInfo,
    render::{Canvas, Executor},
    render_window::{GlobalUserEvent, UserEvent, WindowUserEvent},
    statistics::Statistics,
    types::Vec4f,
};
type Size = crate::types::Size;

pub struct UIState {
    control_ui: ControlUIState,
    module_ui: ModuleUIState,
}

pub struct UILogic {
    main_window: Mutex<WindowId>,
    canvas: Arc<Canvas>,
    ui_state: UIState,
    // executor: Executor,
    gpu_context: GpuContextRef,
    texture_id: egui::TextureId,
}

impl UILogic {
    pub fn new(gpu_context: GpuContextRef) -> Self {
        let canvas = Arc::new(Canvas::new(Size::new(400, 400)));
        let executor = Executor::new(gpu_context.clone());
        let res = Self {
            main_window: unsafe { WindowId::dummy() }.into(),
            canvas: canvas,
            ui_state: UIState {
                control_ui: ControlUIState::default(),
                module_ui: ModuleUIState {
                    modules: executor.list(),
                    select_module: None,
                },
            },
            // executor,
            gpu_context,
            texture_id: egui::TextureId::User(0),
        };
        res
    }

    pub fn set_main_window_id(&mut self, id: WindowId) {
        *self.main_window.get_mut().unwrap() = id;
    }

    pub fn prepare_texture(&self) {
        self.canvas.build_texture(&self.gpu_context.instance());
    }

    pub fn update(
        &mut self,
        ctx: egui::Context,
        statistics: &Statistics,
        event_proxy: &EventLoopProxy<UserEvent>,
    ) {
        let mid = *self.main_window.get_mut().unwrap();
        let gpu = self.gpu_context.instance();
        let state = &mut self.ui_state;
        egui::panel::SidePanel::left("control").show(&ctx, |ui| {
            egui::Grid::new("grid")
                .striped(true)
                .spacing([40.0, 4.0])
                .show(ui, |ui| {
                    control_ui(mid, ui, &mut state.control_ui, statistics, event_proxy)
                })
        });

        egui::Window::new("Memory")
            .open(&mut state.control_ui.memory_opened)
            .show(&ctx, |ui| ctx.memory_ui(ui));
        egui::Window::new("Setting")
            .open(&mut state.control_ui.setting_opened)
            .show(&ctx, |ui| ctx.settings_ui(ui));
        egui::Window::new("Inspection")
            .open(&mut state.control_ui.inspection_opened)
            .show(&ctx, |ui| ctx.inspection_ui(ui));
        egui::Window::new("Texture")
            .open(&mut state.control_ui.texture_opened)
            .show(&ctx, |ui| ctx.texture_ui(ui));

        egui::Window::new("Modules")
            .show(&ctx, |ui| modules_ui(ui, &mut state.module_ui, event_proxy));

        let size = self
            .ui_state
            .control_ui
            .canvas_size
            .unwrap_or(Size::new(100, 100));

        egui::Window::new("Render window").show(&ctx, |ui| {
            ui.image(self.texture_id, egui::vec2(size.x as f32, size.y as f32));
        });
    }

    pub fn finish(
        &self,
        output: &egui::PlatformOutput,
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

    pub fn get_texture<'s>(&'s self, texture_id: &egui::TextureId) -> &'s wgpu::BindGroup {
        if *texture_id == self.texture_id {
            return self.canvas.get_texture().2;
        }
        panic!("invalid texture id");
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
pub struct ControlUIState {
    always_repaint: bool,
    fullscreen: bool,
    clear_color: [f32; 3],
    fps: FpsState,
    setting_opened: bool,
    inspection_opened: bool,
    memory_opened: bool,
    texture_opened: bool,
    canvas_size: Option<Size>,
}

impl Default for ControlUIState {
    fn default() -> Self {
        Self {
            always_repaint: false,
            fullscreen: false,
            clear_color: [0f32, 0f32, 0f32],
            fps: FpsState::default(),
            setting_opened: false,
            inspection_opened: false,
            memory_opened: false,
            texture_opened: false,
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
    let name = "fps limit";
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
    draw_fps(id, ui, state, &event_proxy);
    ui.end_row();

    ui.label("always repaint");
    ui.checkbox(&mut state.always_repaint, "");
    ui.end_row();

    ui.label("full screen");
    if ui.checkbox(&mut state.fullscreen, "").changed() {
        let _ = event_proxy.send_event(UserEvent::Window(
            id,
            WindowUserEvent::FullScreen(state.fullscreen),
        ));
    }
    ui.end_row();

    ui.label("settings");
    ui.collapsing("built-in functions", |ui| {
        if ui.button("setting").clicked() {
            state.setting_opened = !state.setting_opened;
        }
        if ui.button("inspection").clicked() {
            state.inspection_opened = !state.inspection_opened;
        }
        if ui.button("memory").clicked() {
            state.memory_opened = !state.memory_opened;
        }
        if ui.button("texture").clicked() {
            state.texture_opened = !state.texture_opened;
        }
        ui.horizontal(|ui| {
            ui.label("clear color");
            if ui.color_edit_button_rgb(&mut state.clear_color).changed() {
                let _ = event_proxy.send_event(UserEvent::Window(
                    id,
                    WindowUserEvent::ClearColor(Some(Vec4f::new(
                        state.clear_color[0],
                        state.clear_color[1],
                        state.clear_color[2],
                        1.0f32,
                    ))),
                ));
            }
        });
    });
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

    if statistics.changed() || state.always_repaint {
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

    let (inner_rect, pos0, pos1, button_font_id, body_font_id, line_stroke, height) = {
        let fonts = ui.fonts();
        let button_font_id = ui
            .style()
            .text_styles
            .get(&egui::TextStyle::Button)
            .unwrap();
        let body_font_id = ui.style().text_styles.get(&egui::TextStyle::Body).unwrap();
        let row_height0 = fonts.row_height(button_font_id);
        let row_height1 = fonts.row_height(body_font_id);

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
        (
            inner_rect,
            pos0,
            pos1,
            button_font_id.clone(),
            body_font_id.clone(),
            line_stroke,
            height,
        )
    };

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
            widget.rounding,
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
        button_font_id.clone(),
        color0,
    );
    painter.text(
        pos1,
        egui::Align2::LEFT_TOP,
        info.desc,
        body_font_id.clone(),
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
    egui::ScrollArea::vertical().show_viewport(ui, |ui, viewport| {
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
