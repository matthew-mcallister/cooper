use std::ffi::CStr;
use std::hash::{Hash, Hasher};
use std::mem::MaybeUninit;
use std::os::raw::c_char;

use derive_more::{Constructor, From};
use prelude::*;

crate type SmallVec<T, const N: usize> = smallvec::SmallVec<[T; N]>;

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
pub fn clear_color(color: [f32; 4]) -> vk::ClearValue {
    vk::ClearValue {
        color: vk::ClearColorValue {
            float_32: color,
        },
    }
}

#[inline]
pub fn clear_depth(depth: f32) -> vk::ClearValue {
    clear_depth_stencil(depth, 0)
}

#[inline]
pub fn clear_depth_stencil(depth: f32, stencil: u32) -> vk::ClearValue {
    vk::ClearValue {
        depth_stencil: vk::ClearDepthStencilValue { depth, stencil },
    }
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
crate fn as_uninit_slice<T>(src: &[T]) -> &[MaybeUninit<T>] {
    unsafe { &*(src as *const _ as *const _) }
}

#[derive(Constructor, From)]
crate struct DebugIter<I> {
    inner: I,
}

impl<I> std::fmt::Debug for DebugIter<I>
where
    I: IntoIterator + Clone,
    I::Item: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_list().entries(self.inner.clone()).finish()
    }
}

crate unsafe fn debug_cstrs<'a>(ptrs: &'a [*const c_char]) ->
    impl std::fmt::Debug + 'a
{
    DebugIter::new(ptrs.iter().map(|&p| CStr::from_ptr(p)))
}

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

        primitive_enum! {
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

        primitive_enum! {
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

#[macro_export]
macro_rules! set_layout_desc {
    ($(($($binding:tt)*)),*$(,)?) => {
        $crate::DescriptorSetLayoutDesc {
            bindings: smallvec::smallvec![
                $($crate::set_layout_desc!(@binding ($($binding)*)),)*
            ],
            ..Default::default()
        }
    };
    (@binding (
        $binding:expr,
        $type:ident$([$count:expr])?
        $(, $($stages:ident)|+)?
        $(,)?
    )) => {
        $crate::DescriptorSetLayoutBinding {
            binding: $binding,
            ty: $crate::DescriptorType::$type,
            count: { 1 $(; $count)? },
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

macro_rules! set_name {
    ($($var:expr),*$(,)?) => {
        {
            $($var.set_name(stringify!($var));)*
        }
    }
}

// Implements Hash and Eq for a VkDevice-derived object in terms of the
// Vulkan object handle.
macro_rules! impl_device_derived {
    ($name:ident) => {
        impl std::hash::Hash for $name {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.device.hash(state);
                self.inner.hash(state);
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.inner == other.inner && self.device == other.device
            }
        }

        impl Eq for $name {}
    }
}
