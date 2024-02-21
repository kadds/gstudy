type WKC = winit::keyboard::KeyCode;
type CVK = core::event::VirtualKeyCode;

pub fn match_vk(key: winit::keyboard::PhysicalKey) -> CVK {
    match key {
        winit::keyboard::PhysicalKey::Code(c) => match c {
            WKC::Backslash => CVK::Backslash,
            WKC::BracketLeft => CVK::LBracket,
            WKC::BracketRight => CVK::RBracket,
            WKC::Comma => CVK::Comma,
            WKC::Digit0 => CVK::Key0,
            WKC::Digit1 => CVK::Key1,
            WKC::Digit2 => CVK::Key2,
            WKC::Digit3 => CVK::Key3,
            WKC::Digit4 => CVK::Key4,
            WKC::Digit5 => CVK::Key5,
            WKC::Digit6 => CVK::Key6,
            WKC::Digit7 => CVK::Key7,
            WKC::Digit8 => CVK::Key8,
            WKC::Digit9 => CVK::Key9,
            WKC::Equal => CVK::Equals,
            WKC::KeyA => CVK::A,
            WKC::KeyB => CVK::B,
            WKC::KeyC => CVK::C,
            WKC::KeyD => CVK::D,
            WKC::KeyE => CVK::E,
            WKC::KeyF => CVK::F,
            WKC::KeyG => CVK::G,
            WKC::KeyH => CVK::H,
            WKC::KeyI => CVK::I,
            WKC::KeyJ => CVK::J,
            WKC::KeyK => CVK::K,
            WKC::KeyL => CVK::L,
            WKC::KeyM => CVK::M,
            WKC::KeyN => CVK::N,
            WKC::KeyO => CVK::O,
            WKC::KeyP => CVK::P,
            WKC::KeyQ => CVK::Q,
            WKC::KeyR => CVK::R,
            WKC::KeyS => CVK::S,
            WKC::KeyT => CVK::T,
            WKC::KeyU => CVK::U,
            WKC::KeyV => CVK::V,
            WKC::KeyW => CVK::W,
            WKC::KeyX => CVK::X,
            WKC::KeyY => CVK::Y,
            WKC::KeyZ => CVK::Z,
            WKC::Minus => CVK::Minus,
            WKC::Period => CVK::Period,
            // WKC::Quote => todo!(),
            WKC::Semicolon => CVK::Semicolon,
            WKC::Slash => CVK::Slash,
            WKC::AltLeft => CVK::LAlt,
            WKC::AltRight => CVK::RAlt,
            WKC::Backspace => CVK::Back,
            WKC::CapsLock => CVK::Capital,
            WKC::ControlLeft => CVK::LControl,
            WKC::ControlRight => CVK::RControl,
            WKC::Enter => CVK::Return,
            WKC::SuperLeft => CVK::LWin,
            WKC::SuperRight => CVK::RWin,
            WKC::ShiftLeft => CVK::LShift,
            WKC::ShiftRight => CVK::RShift,
            WKC::Space => CVK::Space,
            WKC::Tab => CVK::Tab,
            WKC::Convert => CVK::Convert,
            WKC::KanaMode => CVK::Kana,
            WKC::NonConvert => CVK::NoConvert,
            WKC::Delete => CVK::Delete,
            WKC::End => CVK::End,
            WKC::Home => CVK::Home,
            WKC::Insert => CVK::Insert,
            WKC::PageDown => CVK::PageDown,
            WKC::PageUp => CVK::PageUp,
            WKC::ArrowDown => CVK::Down,
            WKC::ArrowLeft => CVK::Left,
            WKC::ArrowRight => CVK::Right,
            WKC::ArrowUp => CVK::Up,
            WKC::NumLock => CVK::Numlock,
            WKC::Numpad0 => CVK::Numpad0,
            WKC::Numpad1 => CVK::Numpad1,
            WKC::Numpad2 => CVK::Numpad2,
            WKC::Numpad3 => CVK::Numpad3,
            WKC::Numpad4 => CVK::Numpad4,
            WKC::Numpad5 => CVK::Numpad5,
            WKC::Numpad6 => CVK::Numpad6,
            WKC::Numpad7 => CVK::Numpad7,
            WKC::Numpad8 => CVK::Numpad8,
            WKC::Numpad9 => CVK::Numpad9,
            WKC::NumpadAdd => CVK::NumpadAdd,
            WKC::NumpadComma => CVK::NumpadComma,
            WKC::NumpadDecimal => CVK::NumpadDecimal,
            WKC::NumpadDivide => CVK::NumpadDivide,
            WKC::NumpadEnter => CVK::NumpadEnter,
            WKC::NumpadEqual => CVK::NumpadEquals,
            WKC::NumpadMultiply => CVK::NumpadMultiply,
            WKC::NumpadSubtract => CVK::NumpadSubtract,
            WKC::Escape => CVK::Escape,
            WKC::PrintScreen => CVK::Snapshot,
            WKC::ScrollLock => CVK::Scroll,
            WKC::Pause => CVK::Pause,
            WKC::MediaPlayPause => CVK::PlayPause,
            WKC::MediaSelect => CVK::MediaSelect,
            WKC::MediaStop => CVK::MediaStop,
            WKC::Power => CVK::Power,
            WKC::Sleep => CVK::Sleep,
            WKC::F1 => CVK::F1,
            WKC::F2 => CVK::F2,
            WKC::F3 => CVK::F3,
            WKC::F4 => CVK::F4,
            WKC::F5 => CVK::F5,
            WKC::F6 => CVK::F6,
            WKC::F7 => CVK::F7,
            WKC::F8 => CVK::F8,
            WKC::F9 => CVK::F9,
            WKC::F10 => CVK::F10,
            WKC::F11 => CVK::F11,
            WKC::F12 => CVK::F12,
            WKC::F13 => CVK::F13,
            WKC::F14 => CVK::F14,
            WKC::F15 => CVK::F15,
            WKC::F16 => CVK::F16,
            WKC::F17 => CVK::F17,
            WKC::F18 => CVK::F18,
            WKC::F19 => CVK::F19,
            WKC::F20 => CVK::F20,
            WKC::F21 => CVK::F21,
            WKC::F22 => CVK::F22,
            WKC::F23 => CVK::F23,
            WKC::F24 => CVK::F24,
            _ => CVK::Unknown,
        },
        winit::keyboard::PhysicalKey::Unidentified(_) => CVK::Unknown,
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
        _ => None,
    }
}

#[allow(unused)]
pub fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    unsafe {
        ::std::slice::from_raw_parts((p as *const T) as *const u8, ::std::mem::size_of::<T>())
    }
}

#[allow(unused)]
pub fn any_as_u8_slice_array<T: Sized>(p: &[T]) -> &[u8] {
    unsafe { ::std::slice::from_raw_parts(p.as_ptr() as *const u8, std::mem::size_of_val(p)) }
}

#[allow(unused)]
pub fn any_as_x_slice_array<X: Sized, T: Sized>(p: &[T]) -> &[X] {
    unsafe {
        ::std::slice::from_raw_parts(
            p.as_ptr() as *const X,
            std::mem::size_of_val(p) / ::std::mem::size_of::<X>(),
        )
    }
}
