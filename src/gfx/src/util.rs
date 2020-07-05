// TODO: Most/all items in this module should not be exported crate-wide

#![allow(deprecated)]
#![allow(unused_macros)]

use std::cell::Cell;
use std::ffi::CStr;
use std::hash::{Hash, Hasher};
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::os::raw::c_char;
use std::ptr;

use enum_map::{Enum, EnumMap};
use prelude::*;

crate type SmallVec<T, const N: usize> = smallvec::SmallVec<[T; N]>;

#[macro_export]
macro_rules! try_opt {
    ($($body:tt)*) => { (try { $($body)* }: Option<_>) };
}

#[macro_export]
macro_rules! repr_bool {
    (
        $(#[$($meta:meta)*])*
        $vis:vis enum $name:ident {
            $(#[$($meta_f:meta)*])*
            $falsey:ident = false,
            $(#[$($meta_t:meta)*])*
            $truthy:ident = true$(,)?
        }
    ) => {
        $(#[$($meta)*])*
        #[derive(
            Clone, Copy, Debug, enum_map::Enum, Eq, Hash, Ord, PartialEq,
            PartialOrd,
        )]
        #[repr(u8)]
        $vis enum $name {
            $(#[$($meta_f)*])*
            $falsey = 0,
            $(#[$($meta_t)*])*
            $truthy = 1,
        }

        impl From<bool> for $name {
            fn from(b: bool) -> Self {
                unsafe { std::mem::transmute(b) }
            }
        }

        impl From<$name> for bool {
            fn from(x: $name) -> Self {
                unsafe { std::mem::transmute(x) }
            }
        }
    };
}

#[macro_export]
macro_rules! bit {
    ($bit:expr) => {
        (1 << $bit)
    }
}

#[inline]
crate fn bool32(b: bool) -> vk::Bool32 {
    if b { vk::TRUE } else { vk::FALSE }
}

#[inline]
crate fn clear_color(color: [f32; 4]) -> vk::ClearValue {
    vk::ClearValue {
        color: vk::ClearColorValue {
            float_32: color,
        },
    }
}

#[inline]
crate fn clear_depth(depth: f32) -> vk::ClearValue {
    clear_depth_stencil(depth, 0)
}

#[inline]
crate fn clear_depth_stencil(depth: f32, stencil: u32) -> vk::ClearValue {
    vk::ClearValue {
        depth_stencil: vk::ClearDepthStencilValue { depth, stencil },
    }
}

#[inline]
crate fn ptr_eq<T, P: Deref<Target = T>>(this: &P, other: &P) -> bool {
    let this: &T = this.deref();
    let other: &T = other.deref();
    ptr::eq(this, other)
}

#[inline]
crate fn ptr_hash<T, P: Deref<Target = T>, H: Hasher>(this: &P, state: &mut H)
{
    let ptr: &T = this.deref();
    std::ptr::hash(ptr, state);
}

/// If `T` is an aggregate type, it must have *no padding bytes*
/// (including at the end), or this function loses all meaning.
// TODO: comparing byte arrays is maybe slower than comparing primitives
#[inline]
crate fn byte_eq<T>(this: &T, other: &T) -> bool {
    let this = std::slice::from_ref(this).as_bytes();
    let other = std::slice::from_ref(other).as_bytes();
    this == other
}

/// If `T` is an aggregate type, it must have *no padding bytes*
/// (including at the end), or this function loses all meaning.
#[inline]
crate fn byte_hash<T, H: Hasher>(this: &T, state: &mut H) {
    std::slice::from_ref(this).as_bytes().hash(state)
}

/// Remarks on `byte_eq` apply.
#[inline]
crate fn slice_eq<T>(this: &impl AsRef<[T]>, other: &impl AsRef<[T]>) -> bool {
    this.as_ref().as_bytes() == other.as_ref().as_bytes()
}

/// Remarks on `byte_hash` apply.
#[inline]
crate fn slice_hash<T, H: Hasher>(this: &impl AsRef<[T]>, state: &mut H) {
    this.as_ref().as_bytes().hash(state)
}

#[inline]
crate fn as_uninit<T>(src: &T) -> &MaybeUninit<T> {
    unsafe { &*(src as *const _ as *const _) }
}

#[inline]
crate fn as_uninit_slice<T>(src: &[T]) -> &[MaybeUninit<T>] {
    unsafe { &*(src as *const _ as *const _) }
}

#[inline]
crate fn flatten_arrays<T, const N: usize>(arrays: &[[T; N]]) -> &[T] {
    unsafe {
        std::slice::from_raw_parts(
            arrays.as_ptr() as *const T,
            arrays.len() * N,
        )
    }
}

crate struct DebugIter<I: IntoIterator>
    where I::Item: std::fmt::Debug
{
    inner: Cell<Option<I>>,
}

impl<I: IntoIterator> std::fmt::Debug for DebugIter<I>
    where I::Item: std::fmt::Debug
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_list().entries(self.inner.take().unwrap()).finish()
    }
}

impl<I: IntoIterator> DebugIter<I>
    where I::Item: std::fmt::Debug
{
    fn new(iter: I) -> Self {
        Self {
            inner: Cell::new(Some(iter)),
        }
    }
}

crate unsafe fn debug_cstrs<'a>(ptrs: &'a [*const c_char]) ->
    impl std::fmt::Debug + 'a
{
    DebugIter::new(ptrs.iter().map(|&p| CStr::from_ptr(p)))
}

#[macro_export]
macro_rules! primitive_enum {
    (
        @[try_from: $($try_from:ty),*$(,)?]
        @[try_from_error: $try_from_err_ty:ty = $try_from_err_expr:expr]
        @[into: $($into:ty),*$(,)?]
        $(#[$($meta:meta)*])*
        $vis:vis enum $name:ident {
            $($(#[$($mem_meta:meta)*])* $member:ident = $val:expr,)*
        }
    ) => {
        $(#[$($meta)*])*
        $vis enum $name {
            $($(#[$($mem_meta)*])* $member = $val,)*
        }

        $(
            impl From<$name> for $into {
                fn from(val: $name) -> Self {
                    val as _
                }
            }
        )*

        $crate::primitive_enum! {
            @impl_try_from
            @[try_from: $($try_from,)*]
            @[try_from_error: $try_from_err_ty = $try_from_err_expr]
            enum $name { $($member = $val,)* }
        }
    };
    (
        @impl_try_from
        @[try_from: $try_from:ty $(, $try_from_rest:ty)*$(,)?]
        @[try_from_error: $try_from_err_ty:ty = $try_from_err_expr:expr]
        enum $name:ident { $($member:ident = $val:expr,)* }
    ) => {
        impl std::convert::TryFrom<$try_from> for $name {
            type Error = $try_from_err_ty;
            fn try_from(val: $try_from) -> Result<Self, Self::Error> {
                match val {
                    $($val => Ok(Self::$member),)*
                    _ => Err($try_from_err_expr),
                }
            }
        }

        $crate::primitive_enum! {
            @impl_try_from
            @[try_from: $($try_from_rest),*]
            @[try_from_error: $try_from_err_ty = $try_from_err_expr]
            enum $name { $($member = $val,)* }
        }
    };
    (
        @impl_try_from
        @[try_from:]
        @[try_from_error: $try_from_err_ty:ty = $try_from_err_expr:expr]
        enum $name:ident { $($member:ident = $val:expr,)* }
    ) => {};
}

// TODO: This is stupid.
crate fn enum_map<K: Enum<V>, V>(array: K::Array) -> EnumMap<K, V> {
    unsafe {
        let res = std::ptr::read(&array as *const _ as *const EnumMap<K, V>);
        std::mem::forget(array);
        res
    }
}

#[macro_export]
macro_rules! enum_map {
    (
        $($key:expr => $value:expr),*$(,)?
    ) => {
        {
            let mut map = EnumMap::default();
            $(map[$key] = $value;)*
            map
        }
    };
    (
        $($key:expr => $value:expr,)*
        _ => $default:tt,
    ) => {
        {
            let mut map: enum_map::EnumMap<_, _> = (|_| $default).into();
            $(map[$key] = $value;)*
            map
        }
    };
}

#[macro_export]
macro_rules! set_layout_bindings {
    ($(($($binding:tt)*)),*$(,)?) => {
        [$(set_layout_bindings!(@binding ($($binding)*)),)*]
    };
    (@binding (
        $binding:expr, $type:ident$([$count:expr])? $(, $($stages:ident)+)?)
    ) => {
        vk::DescriptorSetLayoutBinding {
            binding: $binding,
            descriptor_type: vk::DescriptorType::$type,
            descriptor_count: { 1 $(; $count)? },
            stage_flags: {
                // TODO: Maybe should be VERTEX | FRAGMENT by default
                (vk::ShaderStageFlags::ALL)
                $(; vk::ShaderStageFlags::empty()
                    $(| vk::ShaderStageFlags::$stages)*)?
            },
            ..Default::default()
        }
    };
}

#[macro_export]
macro_rules! wrap_vk_enum {
    (
        $(#[$($meta:meta)*])*
        $vis:vis enum $name:ident {
            $(
                $(#[$($var_meta:meta)*])*
                $var:ident = $vk_var:ident,
            )*
        }
    ) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        $(#[$($meta)*])*
        $vis enum $name {
            $($(#[$($var_meta)*])* $var,)*
        }

        impl From<$name> for vk::$name {
            fn from(val: $name) -> Self {
                match val {
                    $($name::$var => vk::$name::$vk_var,)*
                }
            }
        }
    }
}

macro_rules! add_to_pnext {
    ($pnext:expr, $struct:expr) => {
        $struct.p_next = $pnext;
        $pnext = &$struct as *const _ as _;
    }
}
