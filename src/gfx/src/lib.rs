#![feature(arbitrary_self_types)]
#![feature(bool_to_option)]
#![feature(crate_visibility_modifier)]
#![feature(seek_convenience)]
#![feature(try_blocks)]

#![allow(dead_code)]
#![allow(unused_imports)]

#[cfg(test)]
macro_rules! test_type {
    () => { crate::testing::VulkanTest }
}

mod descriptor;
mod device;
mod framebuffer;
mod shader;
mod util;
mod vertex;

crate use descriptor::*;
crate use device::*;
crate use framebuffer::*;
crate use shader::*;
crate use util::*;
crate use vertex::*;

mod config;

pub use config::*;

unit::collect_tests![
    descriptor,
    device,
    shader,
    vertex,
];

#[cfg(test)]
mod testing;

#[cfg(test)]
fn main() {
    testing::run_tests();
}
