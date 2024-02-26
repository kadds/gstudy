type EI = egui::CursorIcon;
type EK = egui::Key;
type VK = core::event::VirtualKeyCode;

type WI = winit::window::CursorIcon;

#[allow(unused)]
type WK = core::event::VirtualKeyCode;
#[allow(unused)]

pub fn match_winit_cursor(c: EI) -> Option<WI> {
    Some(match c {
        EI::Default => WI::Default,
        EI::None => return None,
        EI::ContextMenu => WI::ContextMenu,
        EI::Help => WI::Help,
        EI::PointingHand => WI::Pointer,
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

pub type FontFamily = egui::FontFamily;

#[cfg(not(target_arch = "wasm32"))]
pub fn load_font(
    fd: &mut egui::FontDefinitions,
    cache: &rust_fontconfig::FcFontCache,
    name: &str,
    family: FontFamily,
) -> anyhow::Result<()> {
    use rust_fontconfig::FcPattern;
    let font = cache.query(&FcPattern{
        name: Some(name.to_string()),
        ..Default::default()
    }).ok_or(anyhow::anyhow!("query empty"))?;

    let data = std::fs::read(&font.path)?;

    fd.font_data.insert(
        name.to_string(),
        egui::FontData::from_owned(data),
    );
    fd.families
        .entry(family)
        .and_modify(|v| v.insert(0, name.to_string()))
        .or_default();
    Ok(())
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

pub fn match_egui_vkey(k: VK) -> Option<EK> {
    Some(match k {
        VK::Key1 => EK::Num1,
        VK::Key2 => EK::Num2,
        VK::Key3 => EK::Num3,
        VK::Key4 => EK::Num4,
        VK::Key5 => EK::Num5,
        VK::Key6 => EK::Num6,
        VK::Key7 => EK::Num7,
        VK::Key8 => EK::Num8,
        VK::Key9 => EK::Num9,
        VK::Key0 => EK::Num0,
        VK::A => EK::A,
        VK::B => EK::B,
        VK::C => EK::C,
        VK::D => EK::D,
        VK::E => EK::E,
        VK::F => EK::F,
        VK::G => EK::G,
        VK::H => EK::H,
        VK::I => EK::I,
        VK::J => EK::J,
        VK::K => EK::K,
        VK::L => EK::L,
        VK::M => EK::M,
        VK::N => EK::N,
        VK::O => EK::O,
        VK::P => EK::P,
        VK::Q => EK::Q,
        VK::R => EK::R,
        VK::S => EK::S,
        VK::T => EK::T,
        VK::U => EK::U,
        VK::V => EK::V,
        VK::W => EK::W,
        VK::X => EK::X,
        VK::Y => EK::Y,
        VK::Z => EK::Z,
        VK::Escape => EK::Escape,
        VK::Insert => EK::Insert,
        VK::Home => EK::Home,
        VK::Delete => EK::Delete,
        VK::Back => EK::Backspace,
        VK::Return => EK::Enter,
        VK::Space => EK::Space,
        VK::End => EK::End,
        VK::PageDown => EK::PageDown,
        VK::PageUp => EK::PageUp,
        VK::Left => EK::ArrowLeft,
        VK::Up => EK::ArrowUp,
        VK::Right => EK::ArrowRight,
        VK::Down => EK::ArrowDown,
        VK::Numpad0 => EK::Num0,
        VK::Numpad1 => EK::Num1,
        VK::Numpad2 => EK::Num2,
        VK::Numpad3 => EK::Num3,
        VK::Numpad4 => EK::Num4,
        VK::Numpad5 => EK::Num5,
        VK::Numpad6 => EK::Num6,
        VK::Numpad7 => EK::Num7,
        VK::Numpad8 => EK::Num8,
        VK::Numpad9 => EK::Num9,
        VK::Tab => EK::Tab,
        _ => {
            return None;
        }
    })
}
