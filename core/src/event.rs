use std::any::Any;
use std::fmt::Debug;

use crate::context::ResourceRef;
use crate::types::*;

pub trait EventSender: Send {
    fn send_event(&self, ev: Box<dyn Any + Send>);
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
            Self::Pressed => true,
            Self::Released => false,
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
        if let MouseButton::Left = self {
            return true;
        }
        false
    }
    pub fn is_right(&self) -> bool {
        if let MouseButton::Right = self {
            return true;
        }
        false
    }
    pub fn is_middle(&self) -> bool {
        if let MouseButton::Middle = self {
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

    CaptureMouseInputIn,
    CaptureMouseInputOut,
    CaptureKeyboardInputIn,
    CaptureKeyboardInputOut,
}

pub enum Event {
    JustRenderOnce,

    FirstSync,

    PreUpdate(f64),
    // need update window
    Update(f64),
    PostUpdate(f64),

    // render window
    PreRender,
    Render(ResourceRef),
    PostRender,

    Input(InputEvent),

    Resized { logical: Size, physical: Size },

    FullScreen(bool),

    RebuildMaterial,
}

impl Debug for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JustRenderOnce => write!(f, "JustRenderOnce"),
            Self::FirstSync => write!(f, "FirstSync"),
            Self::PreUpdate(arg0) => f.debug_tuple("PreUpdate").field(arg0).finish(),
            Self::Update(arg0) => f.debug_tuple("Update").field(arg0).finish(),
            Self::PostUpdate(arg0) => f.debug_tuple("PostUpdate").field(arg0).finish(),
            Self::PreRender => write!(f, "PreRender"),
            Self::Render(_) => write!(f, "Render"),
            Self::PostRender => write!(f, "PostRender"),
            Self::Input(arg0) => f.debug_tuple("Input").field(arg0).finish(),
            Self::Resized { logical, physical } => f
                .debug_struct("Resized")
                .field("logical", logical)
                .field("physical", physical)
                .finish(),
            Self::FullScreen(arg0) => f.debug_tuple("FullScreen").field(arg0).finish(),
            Self::RebuildMaterial => write!(f, "RebuildMaterial"),
        }
    }
}

pub enum ProcessEventResult {
    Received,
    Consumed,
    ExitLoop,
}

pub trait EventSource {
    fn event_sender(&self) -> &dyn EventSender;
    fn new_event_sender(&self) -> Box<dyn EventSender>;
}

pub trait EventRegistry {
    fn register_processor(&mut self, processor: Box<dyn EventProcessor>);
}

pub trait EventProcessor {
    fn on_event(&mut self, source: &dyn EventSource, event: &dyn Any) -> ProcessEventResult;
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
