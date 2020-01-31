#![allow(unused_macros)]

use std::hash::{Hash, Hasher};
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::ptr;

use derive_more::*;
use prelude::*;

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
            $falsy:ident = false,
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
            $falsy = 0,
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

#[inline]
crate fn as_uninit<T>(src: &T) -> &MaybeUninit<T> {
    unsafe { std::mem::transmute(src) }
}

#[inline]
crate fn as_uninit_slice<T>(src: &[T]) -> &[MaybeUninit<T>] {
    unsafe { std::mem::transmute(src) }
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

#[derive(Clone, Copy, Constructor, Debug, Default, Eq, From, Hash, Into,
    PartialEq)]
crate struct Extent2D {
    crate width: u32,
    crate height: u32,
}

#[derive(Clone, Copy, Constructor, Debug, Default, Eq, From, Hash, Into,
    PartialEq)]
crate struct Extent3D {
    crate width: u32,
    crate height: u32,
    crate depth: u32,
}

impl From<Extent3D> for Extent2D {
    fn from(extent: Extent3D) -> Self {
        (extent.width, extent.height).into()
    }
}

impl From<Extent2D> for vk::Extent2D {
    fn from(Extent2D { width, height }: Extent2D) -> Self {
        Self { width, height }
    }
}

impl From<vk::Extent2D> for Extent2D {
    fn from(vk::Extent2D { width, height }: vk::Extent2D) -> Self {
        Self { width, height }
    }
}

impl From<Extent2D> for Extent3D {
    fn from(extent: Extent2D) -> Self {
        (extent.width, extent.height, 1).into()
    }
}

impl From<(u32, u32)> for Extent3D {
    fn from((width, height): (u32, u32)) -> Self {
        (width, height, 1).into()
    }
}

impl From<Extent3D> for vk::Extent3D {
    fn from(Extent3D { width, height, depth }: Extent3D) -> Self {
        Self { width, height, depth }
    }
}

impl From<vk::Extent3D> for Extent3D {
    fn from(vk::Extent3D { width, height, depth }: vk::Extent3D) -> Self {
        Self { width, height, depth }
    }
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
