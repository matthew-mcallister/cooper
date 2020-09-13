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

#[macro_use]
mod macros;

//pub mod bbox;
//pub mod matrix;
pub mod vector;

//pub use bbox::*;
//pub use matrix::*;
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
