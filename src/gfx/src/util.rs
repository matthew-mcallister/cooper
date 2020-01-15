#![allow(unused_macros)]

use std::hash::{Hash, Hasher};
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::ptr;
use std::sync::Arc;

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
        #[allow(path_statements)]
        vk::DescriptorSetLayoutBinding {
            binding: $binding,
            descriptor_type: vk::DescriptorType::$type,
            descriptor_count: { 1 $(; $count)? },
            stage_flags: {
                vk::ShaderStageFlags::ALL
                $(; vk::ShaderStageFlags::empty()
                    $(| vk::ShaderStageFlags::$stages)*)?
            },
            ..Default::default()
        }
    };
}
