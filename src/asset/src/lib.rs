#![feature(
    backtrace,
    const_generics,
    crate_visibility_modifier,
    entry_insert,
    exact_size_is_empty,
    try_blocks,
    try_trait,
    type_ascription,
)]
#![allow(incomplete_features)]
#![allow(clippy::float_cmp)]

unsafe fn extend_lt<T>(val: &T) -> &'static T { &*(val as *const T) }

// borrowck is trash
macro_rules! try_return_elem {
    ($src:expr, $key:expr$(,)?) => {
        let src = unsafe { $crate::extend_lt($src) };
        if let Some(val) = src.get($key) {
            return Ok(val);
        }
    };
}

#[macro_use]
mod error;

mod asset;
mod gltf;
mod scene;

pub use asset::*;
pub use error::*;
pub use scene::*;
