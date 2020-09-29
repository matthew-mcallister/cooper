#![feature(
    array_value_iter,
    bool_to_option,
    const_generics,
    crate_visibility_modifier,
    iterator_fold_self,
    maybe_uninit_extra,
    maybe_uninit_uninit_array,
    trait_alias,
)]
#![allow(incomplete_features)]

use std::ops::*;

#[macro_use]
mod macros;

pub mod bbox;
pub mod matrix;
pub mod quaternion;
pub mod uvector;
pub mod vector;

pub use bbox::*;
pub use matrix::*;
pub use quaternion::*;
pub use uvector::*;
pub use vector::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InfSupResult<T> {
    Empty,
    Singleton(T),
    InfSup(T, T),
}

pub trait InfSup<A = Self>: Sized {
    fn inf<I>(iter: I) -> Option<Self>
        where I: Iterator<Item = A>;
    fn sup<I>(iter: I) -> Option<Self>
        where I: Iterator<Item = A>;
    fn inf_sup<I>(iter: I) -> InfSupResult<Self>
        where I: Iterator<Item = A>;
}

pub trait MathItertools: Iterator {
    fn inf<T: InfSup<Self::Item>>(self) -> Option<T>;
    fn sup<T: InfSup<Self::Item>>(self) -> Option<T>;
    fn inf_sup<T: InfSup<Self::Item>>(self) -> InfSupResult<T>;
}

impl<I: Iterator> MathItertools for I {
    fn inf<T: InfSup<Self::Item>>(self) -> Option<T> {
        <T as InfSup<Self::Item>>::inf(self)
    }

    fn sup<T: InfSup<Self::Item>>(self) -> Option<T> {
        <T as InfSup<Self::Item>>::sup(self)
    }

    fn inf_sup<T: InfSup<Self::Item>>(self) -> InfSupResult<T> {
        <T as InfSup<Self::Item>>::inf_sup(self)
    }
}

pub trait VectorOps<F>
    = Sized
    + Neg<Output = Self>
    + Add<Self, Output = Self>
    + AddAssign<Self>
    + Sub<Self, Output = Self>
    + SubAssign<Self>
    + Mul<Self, Output = Self>
    + MulAssign<Self>
    + Mul<F, Output = Self>
    + MulAssign<F>
    + Div<Self, Output = Self>
    + DivAssign<Self>
    + Div<F, Output = Self>
    + DivAssign<F>
    ;
