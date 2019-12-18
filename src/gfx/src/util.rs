use std::hash::Hasher;
use std::sync::Arc;

#[inline]
crate fn bool32(b: bool) -> vk::Bool32 {
    if b { vk::TRUE } else { vk::FALSE }
}

#[inline]
crate fn clear_color(color: [f32; 4]) -> vk::ClearValue {
            vk::ClearValue {
        color: vk::ClearColorValue {
            float_32: color,
        },
    }
}

/// Like `Arc::ptr_eq`, but for the `Hash` trait.
#[inline]
crate fn arc_ptr_hash<T: ?Sized, H: Hasher>(this: &Arc<T>, state: &mut H) {
    let ptr: &T = &*this;
    std::ptr::hash(ptr, state);
}
