use std::ffi::CStr;
use std::hash::{Hash, Hasher};
use std::mem::MaybeUninit;
use std::os::raw::c_char;

use derive_more::{Constructor, From};

macro_rules! tryopt {
    ($($body:tt)*) => { (try { $($body)* }: Option<_>) }
}

/// Returns the smallest multiple of `alignment` that is `>= offset`.
#[inline(always)]
pub(crate) fn align<T: Copy + base::num::PrimInt>(alignment: T, offset: T) -> T {
    ((offset + alignment - T::one()) / alignment) * alignment
}

pub(crate) trait SliceExt {
    type Target: Sized;

    /// Casts a slice to a byte array.
    fn as_bytes(&self) -> &[u8];

    /// Casts a slice to a mutable byte array.
    fn as_bytes_mut(&mut self) -> &mut [u8];

    /// Converts a slice to a *non-dangling* pointer. This means that,
    /// if the slice has length zero, the returned pointer is NULL.
    /// Though it is hardly undocumented, this is not the case for
    /// `slice::as_ptr`.
    fn c_ptr(&self) -> *const Self::Target;
}

impl<T> SliceExt for [T] {
    type Target = T;

    #[inline(always)]
    fn as_bytes(&self) -> &[u8] {
        let len = self.len() * std::mem::size_of::<T>();
        unsafe { std::slice::from_raw_parts(self as *const [T] as _, len) }
    }

    #[inline(always)]
    fn as_bytes_mut(&mut self) -> &mut [u8] {
        let len = self.len() * std::mem::size_of::<T>();
        unsafe { std::slice::from_raw_parts_mut(self as *mut [T] as _, len) }
    }

    #[inline(always)]
    fn c_ptr(&self) -> *const Self::Target {
        if self.is_empty() {
            std::ptr::null()
        } else {
            self.as_ptr()
        }
    }
}

#[macro_export]
macro_rules! c_str {
    ($($str:expr),*$(,)*) => {
        concat!($($str,)* "\0") as *const str as *const std::os::raw::c_char
    };
}

pub trait AsPtr {
    type Target;

    fn as_ptr(self) -> *const Self::Target;
}

impl<'a, T> AsPtr for Option<&'a T> {
    type Target = T;

    #[inline(always)]
    fn as_ptr(self) -> *const Self::Target {
        unsafe { std::mem::transmute(self) }
    }
}

pub(crate) type SmallVec<T, const N: usize> = smallvec::SmallVec<[T; N]>;

macro_rules! bit {
    ($bit:expr) => {
        (1 << $bit)
    };
}

#[inline]
pub(crate) fn bool32(b: bool) -> vk::Bool32 {
    if b {
        vk::TRUE
    } else {
        vk::FALSE
    }
}

#[inline]
pub fn clear_color(color: [f32; 4]) -> vk::ClearValue {
    vk::ClearValue {
        color: vk::ClearColorValue { float_32: color },
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
pub(crate) fn byte_eq<T>(this: &T, other: &T) -> bool {
    let this = std::slice::from_ref(this).as_bytes();
    let other = std::slice::from_ref(other).as_bytes();
    this == other
}

/// If `T` is an aggregate type, it must have *no padding bytes*
/// (including at the end), or this function loses all meaning.
#[inline]
pub(crate) fn byte_hash<T, H: Hasher>(this: &T, state: &mut H) {
    std::slice::from_ref(this).as_bytes().hash(state)
}

#[inline]
pub(crate) fn as_uninit_slice<T>(src: &[T]) -> &[MaybeUninit<T>] {
    unsafe { &*(src as *const _ as *const _) }
}

#[derive(Constructor, From)]
pub(crate) struct DebugIter<I> {
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

pub(crate) unsafe fn debug_cstrs<'a>(ptrs: &'a [*const c_char]) -> impl std::fmt::Debug + 'a {
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
    };
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
    };
}
