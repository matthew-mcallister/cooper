#![feature(
    array_value_iter,
    bool_to_option,
    const_generics,
    iterator_fold_self,
    trait_alias,
)]

#![allow(incomplete_features)]

#[macro_use]
mod macros;

pub mod bbox;
pub mod matrix;
mod traits;
pub mod vector;

pub use bbox::*;
pub use matrix::*;
pub use traits::*;
pub use vector::*;
