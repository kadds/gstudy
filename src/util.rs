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
