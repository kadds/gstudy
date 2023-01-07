use std::sync::Arc;

use crate::backends::WGPUBackend;
use crate::types::*;
// use winit::{
//     dpi::PhysicalPosition,
//     event::{
//         DeviceEvent, DeviceId, ElementState, KeyboardInput, ModifiersState, MouseButton,
//         MouseScrollDelta, TouchPhase,
//     },
//     window::CursorIcon,
// };

pub trait EventSender: Send {
    fn send_event(&self, ev: Event);
}

#[derive(Debug, Clone)]
pub enum Modifiers {
    Ctrl,
    Win,
    Alt,
    Shift,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ElementState {
    Pressed,
    Released,
}

impl ElementState {
    pub fn is_pressed(&self) -> bool {
        match self {
            Pressed => true,
            Released => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModifiersState {
    pub ctrl: bool,
    pub win: bool,
    pub alt: bool,
    pub shift: bool,
}

#[derive(Debug, Clone)]
pub struct MouseState {
    pub left: bool,
    pub right: bool,
    pub middle: bool,
}

#[derive(Debug, Clone)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    pub fn is_left(&self) -> bool {
        if let Left = self {
            return true;
        }
        false
    }
    pub fn is_right(&self) -> bool {
        if let Right = self {
            return true;
        }
        false
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum CustomEvent {
    OpenUrl(String),
    Exit,

    CanvasResize(Size),
    ModuleChanged(&'static str),
    ClearColor(Option<Color>),

    Loading(String),
    Loaded(u64),
}

#[derive(Debug, Clone)]
pub struct KeyboardInput {
    pub state: ElementState,
    pub vk: VirtualKeyCode,
}

#[derive(Debug, Clone)]
pub enum InputEvent {
    KeyboardInput(KeyboardInput),
    ModifiersChanged(ModifiersState),

    CursorMoved {
        logical: Vec2f,
        physical: Vec2f,
    },

    ReceivedCharacter(char),
    ReceivedString(String),

    CursorEntered,

    CursorLeft,

    MouseWheel {
        delta: Vec3f,
    },

    MouseInput {
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

    Input(InputEvent),

    Theme(Theme),

    Resized { logical: Size, physical: Size },

    Moved(Size),

    CloseRequested,

    Focused(bool),

    UpdateCursor(egui::CursorIcon),

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
    // fn window(&self) -> &winit::window::Window;
    fn backend(&self) -> &WGPUBackend;
    fn event_sender(&self) -> &dyn EventSender;
    fn new_event_sender(&self) -> Box<dyn EventSender>;
}

pub trait EventProcessor {
    fn on_event(&mut self, source: &dyn EventSource, event: &Event) -> ProcessEventResult;
}

#[derive(Debug, Hash, Ord, PartialOrd, PartialEq, Eq, Clone, Copy)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum VirtualKeyCode {
    /// The '1' key over the letters.
    Key1,
    /// The '2' key over the letters.
    Key2,
    /// The '3' key over the letters.
    Key3,
    /// The '4' key over the letters.
    Key4,
    /// The '5' key over the letters.
    Key5,
    /// The '6' key over the letters.
    Key6,
    /// The '7' key over the letters.
    Key7,
    /// The '8' key over the letters.
    Key8,
    /// The '9' key over the letters.
    Key9,
    /// The '0' key over the 'O' and 'P' keys.
    Key0,

    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    /// The Escape key, next to F1.
    Escape,

    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,

    /// Print Screen/SysRq.
    Snapshot,
    /// Scroll Lock.
    Scroll,
    /// Pause/Break key, next to Scroll lock.
    Pause,

    /// `Insert`, next to Backspace.
    Insert,
    Home,
    Delete,
    End,
    PageDown,
    PageUp,

    Left,
    Up,
    Right,
    Down,

    /// The Backspace key, right over Enter.
    // TODO: rename
    Back,
    /// The Enter key.
    Return,
    /// The space bar.
    Space,

    /// The "Compose" key on Linux.
    Compose,

    Caret,

    Numlock,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadAdd,
    NumpadDivide,
    NumpadDecimal,
    NumpadComma,
    NumpadEnter,
    NumpadEquals,
    NumpadMultiply,
    NumpadSubtract,

    AbntC1,
    AbntC2,
    Apostrophe,
    Apps,
    Asterisk,
    At,
    Ax,
    Backslash,
    Calculator,
    Capital,
    Colon,
    Comma,
    Convert,
    Equals,
    Grave,
    Kana,
    Kanji,
    LAlt,
    LBracket,
    LControl,
    LShift,
    LWin,
    Mail,
    MediaSelect,
    MediaStop,
    Minus,
    Mute,
    MyComputer,
    // also called "Next"
    NavigateForward,
    // also called "Prior"
    NavigateBackward,
    NextTrack,
    NoConvert,
    OEM102,
    Period,
    PlayPause,
    Plus,
    Power,
    PrevTrack,
    RAlt,
    RBracket,
    RControl,
    RShift,
    RWin,
    Semicolon,
    Slash,
    Sleep,
    Stop,
    Sysrq,
    Tab,
    Underline,
    Unlabeled,
    VolumeDown,
    VolumeUp,
    Wake,
    WebBack,
    WebFavorites,
    WebForward,
    WebHome,
    WebRefresh,
    WebSearch,
    WebStop,
    Yen,
    Copy,
    Paste,
    Cut,

    Unknown,
}

type WK = VirtualKeyCode;
type EK = egui::Key;

pub fn match_egui_key(k: WK) -> Option<EK> {
    Some(match k {
        WK::Key1 => EK::Num1,
        WK::Key2 => EK::Num2,
        WK::Key3 => EK::Num3,
        WK::Key4 => EK::Num4,
        WK::Key5 => EK::Num5,
        WK::Key6 => EK::Num6,
        WK::Key7 => EK::Num7,
        WK::Key8 => EK::Num8,
        WK::Key9 => EK::Num9,
        WK::Key0 => EK::Num0,
        WK::A => EK::A,
        WK::B => EK::B,
        WK::C => EK::C,
        WK::D => EK::D,
        WK::E => EK::E,
        WK::F => EK::F,
        WK::G => EK::G,
        WK::H => EK::H,
        WK::I => EK::I,
        WK::J => EK::J,
        WK::K => EK::K,
        WK::L => EK::L,
        WK::M => EK::M,
        WK::N => EK::N,
        WK::O => EK::O,
        WK::P => EK::P,
        WK::Q => EK::Q,
        WK::R => EK::R,
        WK::S => EK::S,
        WK::T => EK::T,
        WK::U => EK::U,
        WK::V => EK::V,
        WK::W => EK::W,
        WK::X => EK::X,
        WK::Y => EK::Y,
        WK::Z => EK::Z,
        WK::Escape => EK::Escape,
        WK::Insert => EK::Insert,
        WK::Home => EK::Home,
        WK::Delete => EK::Delete,
        WK::Back => EK::Backspace,
        WK::Return => EK::Enter,
        WK::Space => EK::Space,
        WK::End => EK::End,
        WK::PageDown => EK::PageDown,
        WK::PageUp => EK::PageUp,
        WK::Left => EK::ArrowLeft,
        WK::Up => EK::ArrowUp,
        WK::Right => EK::ArrowRight,
        WK::Down => EK::ArrowDown,
        WK::Numpad0 => EK::Num0,
        WK::Numpad1 => EK::Num1,
        WK::Numpad2 => EK::Num2,
        WK::Numpad3 => EK::Num3,
        WK::Numpad4 => EK::Num4,
        WK::Numpad5 => EK::Num5,
        WK::Numpad6 => EK::Num6,
        WK::Numpad7 => EK::Num7,
        WK::Numpad8 => EK::Num8,
        WK::Numpad9 => EK::Num9,
        WK::Tab => EK::Tab,
        _ => {
            return None;
        }
    })
}
