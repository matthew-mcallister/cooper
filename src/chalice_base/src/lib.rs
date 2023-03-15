#![feature(hash_raw_entry, trait_alias, try_blocks)]

#[macro_use]
mod ops;
#[macro_use]
mod enum_vec;
#[macro_use]
mod macros;
#[macro_use]
pub mod bitfield;

mod by_ptr;
pub mod num;
mod partial_enum_map;

pub use by_ptr::*;
pub use enum_vec::*;
pub use partial_enum_map::*;

pub fn slice_to_bytes<T: Copy>(slice: &[T]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            slice.as_ptr() as *const u8,
            std::mem::size_of::<T>() * slice.len(),
        )
    }
}
