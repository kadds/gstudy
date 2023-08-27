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
