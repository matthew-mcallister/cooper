use std::fmt;

use derive_more::*;
use num_traits as num;

#[macro_export]
macro_rules! c_str {
    ($($str:expr),*) => {
        c_str!($($str,)*)
    };
    ($($str:expr,)*) => {
        concat!($($str,)* "\0") as *const str as *const std::os::raw::c_char
    };
}

// Returns the smallest multiple of `alignment` that is `>= offset`.
#[inline(always)]
pub fn align<T: Copy + num::Num>(alignment: T, offset: T) -> T {
    ((offset + alignment - num::one()) / alignment) * alignment
}

// A.k.a. guard
#[inline(always)]
pub fn opt(cond: bool) -> Option<()> {
    if cond { Some(()) } else { None }
}

// Vexing that this isn't in std
#[inline(always)]
pub fn slice_to_bytes<T: Sized>(slice: &[T]) -> &[u8] {
    let len = slice.len() * std::mem::size_of::<T>();
    unsafe { std::slice::from_raw_parts(slice as *const [T] as _, len) }
}

#[inline(always)]
pub fn uninit_buffer(size: usize) -> Vec<u8> {
    let mut res = Vec::with_capacity(size);
    unsafe { res.set_len(size); }
    res
}

pub type AnyError = Box<dyn std::error::Error>;

#[derive(Clone, Constructor, Copy, Debug)]
pub struct EnumValueError {
    value: u32,
}

impl fmt::Display for EnumValueError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "unrecognized enum value: {}", self.value)
    }
}

impl std::error::Error for EnumValueError {}

#[macro_export]
macro_rules! impl_enum {
    ($name:ident[$type:ident] { $($member:ident = $value:expr,)* }) => {
        #[repr($type)]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum $name {
            $($member = $value,)*
        }
        impl $name {
            const VALUES: &'static [$name] = &[$($name::$member,)*];
        }
        impl std::convert::TryFrom<$type> for $name {
            type Error = EnumValueError;
            fn try_from(value: $type) -> std::result::Result<Self, Self::Error>
            {
                if $name::VALUES.iter().any(|&e| e as $type == value) {
                    Ok(unsafe { std::mem::transmute(value) })
                } else {
                    Err(EnumValueError::new(value as _))
                }
            }
        }
    }
}

#[macro_export]
macro_rules! impl_default {
    ($name:ident, $val:expr) => {
        impl std::default::Default for $name {
            fn default() -> Self {
                $val
            }
        }
    }
}

pub trait ResultExt<T, E> {
    fn on_err(self, f: impl FnOnce(&E)) -> Self;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    #[inline(always)]
    fn on_err(self, f: impl FnOnce(&E)) -> Self {
        self.as_ref().err().map(f);
        self
    }
}
