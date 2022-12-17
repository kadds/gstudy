use crate::types::*;
use crate::{model::Model, render::Scene};
use winit::{
    dpi::PhysicalPosition,
    event::{
        DeviceEvent, DeviceId, ElementState, KeyboardInput, ModifiersState, MouseButton,
        MouseScrollDelta, TouchPhase,
    },
    window::CursorIcon,
};

#[allow(dead_code)]
#[derive(Debug)]
pub enum CustomEvent {
    OpenUrl(String),
    Exit,

    CanvasResize(Size),
    ModuleChanged(&'static str),
    ClearColor(Option<Color>),

    Loading(String),
    Loaded(Scene),
}

#[derive(Debug, Clone)]
pub enum InputEvent {
    KeyboardInput {
        device_id: DeviceId,
        input: KeyboardInput,
        is_synthetic: bool,
    },
    ModifiersChanged(ModifiersState),

    CursorMoved {
        device_id: DeviceId,
        position: PhysicalPosition<f64>,
    },

    ReceivedCharacter(char),
    ReceivedString(String),

    CursorEntered {
        device_id: DeviceId,
    },

    CursorLeft {
        device_id: DeviceId,
    },

    MouseWheel {
        device_id: DeviceId,
        delta: MouseScrollDelta,
        phase: TouchPhase,
    },

    MouseInput {
        device_id: DeviceId,
        state: ElementState,
        button: MouseButton,
    },
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum Theme {
    Light,
    Dark,
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum Event {
    JustRenderOnce,

    // need update window
    Update(f64),
    // render window
    Render,

    CustomEvent(CustomEvent),

    // raw input event
    RawInput(DeviceEvent),

    Input(InputEvent),

    Theme(Theme),

    Resized(Size),

    Moved(Size),

    CloseRequested,

    Focused(bool),

    UpdateCursor(CursorIcon),

    UpdateImePosition((u32, u32)),

    FullScreen(bool),

    ScaleFactorChanged(f64),
}

pub enum ProcessEventResult {
    Received,
    Consumed,
    ExitLoop,
}

pub trait EventSource {
    fn window(&self) -> &winit::window::Window;
    fn backend(&self) -> &crate::backends::WGPUBackend;
    fn event_proxy(&self) -> winit::event_loop::EventLoopProxy<Event>;
}

pub trait EventProcessor {
    fn on_event(&mut self, source: &dyn EventSource, event: &Event) -> ProcessEventResult;
}
