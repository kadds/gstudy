type WI = winit::window::CursorIcon;
type EI = egui::CursorIcon;

type WK = winit::event::VirtualKeyCode;
type EK = egui::Key;

pub fn match_winit_cursor(c: EI) -> Option<WI> {
    Some(match c {
        EI::Default => WI::Default,
        EI::None => return None,
        EI::ContextMenu => WI::ContextMenu,
        EI::Help => WI::Help,
        EI::PointingHand => WI::Hand,
        EI::Progress => WI::Progress,
        EI::Wait => WI::Wait,
        EI::Cell => WI::Cell,
        EI::Crosshair => WI::Crosshair,
        EI::Text => WI::Text,
        EI::VerticalText => WI::VerticalText,
        EI::Alias => WI::Alias,
        EI::Copy => WI::Copy,
        EI::Move => WI::Move,
        EI::NoDrop => WI::NoDrop,
        EI::NotAllowed => WI::NotAllowed,
        EI::Grab => WI::Grab,
        EI::Grabbing => WI::Grabbing,
        EI::AllScroll => WI::AllScroll,
        EI::ResizeHorizontal => WI::EwResize,
        EI::ResizeNeSw => WI::NeswResize,
        EI::ResizeNwSe => WI::NwseResize,
        EI::ResizeVertical => WI::NsResize,
        EI::ResizeEast => WI::EResize,
        EI::ResizeSouthEast => WI::SeResize,
        EI::ResizeSouth => WI::SResize,
        EI::ResizeSouthWest => WI::SwResize,
        EI::ResizeWest => WI::WResize,
        EI::ResizeNorthWest => WI::NwResize,
        EI::ResizeNorth => WI::NResize,
        EI::ResizeNorthEast => WI::NeResize,
        EI::ResizeColumn => WI::ColResize,
        EI::ResizeRow => WI::RowResize,

        EI::ZoomIn => WI::ZoomIn,
        EI::ZoomOut => WI::ZoomOut,
    })
}

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

pub fn match_vk(input: Option<winit::event::VirtualKeyCode>) -> core::event::VirtualKeyCode {
    if let Some(v) = input {
        unsafe { std::mem::transmute(v) }
    } else {
        core::event::VirtualKeyCode::Unknown
    }
}

pub fn match_state(state: winit::event::ElementState) -> core::event::ElementState {
    match state {
        winit::event::ElementState::Pressed => core::event::ElementState::Pressed,
        winit::event::ElementState::Released => core::event::ElementState::Released,
    }
}

pub fn match_button(button: winit::event::MouseButton) -> Option<core::event::MouseButton> {
    match button {
        winit::event::MouseButton::Left => Some(core::event::MouseButton::Left),
        winit::event::MouseButton::Right => Some(core::event::MouseButton::Right),
        winit::event::MouseButton::Middle => Some(core::event::MouseButton::Middle),
        winit::event::MouseButton::Other(_) => None,
    }
}

pub fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    unsafe {
        ::std::slice::from_raw_parts((p as *const T) as *const u8, ::std::mem::size_of::<T>())
    }
}
pub fn any_as_u8_slice_array<T: Sized>(p: &[T]) -> &[u8] {
    unsafe {
        ::std::slice::from_raw_parts(
            (p.as_ptr() as *const T) as *const u8,
            ::std::mem::size_of::<T>() * p.len(),
        )
    }
}
pub fn any_as_x_slice_array<X: Sized, T: Sized>(p: &[T]) -> &[X] {
    unsafe {
        ::std::slice::from_raw_parts(
            (p.as_ptr() as *const T) as *const X,
            p.len() * ::std::mem::size_of::<T>() / ::std::mem::size_of::<X>(),
        )
    }
}
