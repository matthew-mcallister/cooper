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

/// Implements the dot product for vectors.
pub trait Dot<Rhs = Self> {
    type Output;
    fn dot(self, rhs: Rhs) -> Self::Output;
}

/// Implements the cross product for vectors.
pub trait Cross<Rhs = Self> {
    type Output;
    fn cross(self, rhs: Rhs) -> Self::Output;
}

pub fn dot<Lhs, Rhs>(lhs: Lhs, rhs: Rhs) -> Lhs::Output
    where Lhs: Dot<Rhs>,
{
    lhs.dot(rhs)
}

pub fn cross<Lhs, Rhs>(lhs: Lhs, rhs: Rhs) -> Lhs::Output
    where Lhs: Cross<Rhs>,
{
    lhs.cross(rhs)
}
