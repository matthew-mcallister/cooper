#![feature(
    backtrace,
    const_generics,
    crate_visibility_modifier,
    entry_insert,
    exact_size_is_empty,
    try_blocks,
    try_trait,
    type_ascription,
    vec_into_raw_parts,
)]
#![allow(incomplete_features)]
#![allow(clippy::float_cmp)]

#[macro_use]
mod error;

mod asset;
mod gltf;
mod scene;

pub use asset::*;
pub use error::*;
pub use scene::*;

fn vec_to_bytes<T>(vec: Vec<T>) -> Vec<u8> {
    unsafe {
        let (ptr, len, cap) = vec.into_raw_parts();
        Vec::from_raw_parts(
            ptr as *mut u8,
            // TODO: Probably should panic on overflow
            len * std::mem::size_of::<T>(),
            cap * std::mem::size_of::<T>(),
        )
    }
}
