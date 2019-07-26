#![feature(crate_visibility_modifier)]
#![feature(seek_convenience)]
#![feature(try_blocks)]
use std::fmt;

macro_rules! c_str {
    ($str:expr) => {
        concat!($str, "\0") as *const str as *const std::os::raw::c_char
    }
}

macro_rules! include_shader {
    ($name:expr) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            concat!("/generated/shaders/", $name),
        ))
    }
}

mod descriptor;
mod frame;
mod init;
mod master;
mod memory;
mod object;
mod render_path;
mod sprite;
mod stats;
mod texture;

pub use descriptor::*;
pub use frame::*;
pub use init::*;
pub use master::*;
pub use memory::*;
pub use object::*;
pub use render_path::*;
pub use sprite::*;
pub use stats::*;
pub use texture::*;

#[inline(always)]
#[allow(dead_code)]
crate fn align(alignment: usize, offset: usize) -> usize {
    ((offset + alignment - 1) / alignment) * alignment
}

#[inline(always)]
crate fn align_64(alignment: u64, offset: u64) -> u64 {
    ((offset + alignment - 1) / alignment) * alignment
}

#[inline(always)]
crate fn opt(cond: bool) -> Option<()> {
    if cond { Some(()) } else { None }
}

// Vexing that this isn't in std
#[inline(always)]
crate fn slice_to_bytes<T: Sized>(slice: &[T]) -> &[u8] {
    let len = slice.len() * std::mem::size_of::<T>();
    unsafe { std::slice::from_raw_parts(slice as *const [T] as _, len) }
}

crate type AnyError = Box<dyn std::error::Error>;

#[derive(Clone, Copy, Debug)]
crate struct EnumValueError {
    value: u32,
}

impl fmt::Display for EnumValueError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "unrecognized enum value: {}", self.value)
    }
}

impl std::error::Error for EnumValueError {}
