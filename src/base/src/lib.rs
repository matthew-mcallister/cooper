#![feature(crate_visibility_modifier)]
#![feature(manually_drop_take)]
#![feature(trait_alias)]
#![feature(try_blocks)]
#![feature(untagged_unions)]

#[macro_use]
mod enum_vec;
#[macro_use]
mod ops;
#[macro_use]
mod macros;
#[macro_use]
pub mod bitfield;

mod name;
mod sentinel;
pub mod node;
pub mod pool;
pub mod request;

pub use enum_vec::*;
pub use name::*;
pub use sentinel::*;