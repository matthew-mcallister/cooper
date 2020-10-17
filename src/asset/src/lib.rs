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

#[macro_use]
mod error;

mod asset;
mod gltf;
mod scene;

pub use asset::*;
pub use error::*;
pub use scene::*;
