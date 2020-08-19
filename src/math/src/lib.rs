#![feature(
    array_value_iter,
    bool_to_option,
    const_generics,
    iterator_fold_self,
    maybe_uninit_extra,
    maybe_uninit_uninit_array,
    trait_alias,
)]
#![allow(incomplete_features)]

use std::mem::MaybeUninit;

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

fn uninit_slice<T, const N: usize>(array: &MaybeUninit<[T; N]>) ->
    &[MaybeUninit<T>; N]
{
    unsafe { &*(array as *const MaybeUninit<_> as *const [MaybeUninit<T>; N]) }
}

fn uninit_slice_mut<T, const N: usize>(array: &mut MaybeUninit<[T; N]>)
    -> &mut [MaybeUninit<T>; N]
{
    unsafe { &mut *(array as *mut MaybeUninit<_> as *mut [MaybeUninit<T>; N]) }
}
