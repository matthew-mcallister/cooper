#![feature(arbitrary_self_types)]
#![feature(bool_to_option)]
#![feature(crate_visibility_modifier)]
#![feature(seek_convenience)]
#![feature(try_blocks)]
#![feature(type_ascription)]

#![allow(dead_code)]
#![allow(unused_imports)]

#[cfg(test)]
macro_rules! test_type {
    () => { crate::testing::VulkanTest }
}

#[macro_use]
mod util;
crate use util::*;

mod descriptor;
mod device;
mod framebuffer;
mod pipeline;
mod render_pass;
mod shader;
mod vertex;

crate use descriptor::*;
crate use device::*;
crate use framebuffer::*;
crate use pipeline::*;
crate use render_pass::*;
crate use shader::*;
crate use vertex::*;

mod config;

pub use config::*;

unit::collect_tests![
    descriptor,
    device,
    pipeline,
    render_pass,
    shader,
    vertex,
];

#[cfg(test)]
mod testing;

#[cfg(test)]
fn main() {
    testing::run_tests();
}
