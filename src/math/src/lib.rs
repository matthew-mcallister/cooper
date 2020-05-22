#![feature(array_value_iter)]
#![feature(const_generics)]
#![feature(trait_alias)]

#![allow(incomplete_features)]

#[macro_use]
mod macros;

mod matrix;
mod vector;

pub use matrix::*;
pub use vector::*;
