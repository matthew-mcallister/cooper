#![feature(arbitrary_self_types)]
#![feature(bool_to_option)]
#![feature(cow_is_borrowed)]
#![feature(crate_visibility_modifier)]
#![feature(manually_drop_take)]
#![feature(maybe_uninit_extra)]
#![feature(try_blocks)]
#![feature(type_ascription)]

#![allow(dead_code)]

#[cfg(test)]
macro_rules! test_type {
    () => { crate::testing::VulkanTest }
}

#[macro_use]
mod util;
crate use util::*;

mod device;
mod format;
mod framebuffer;
mod global;
mod image;
mod pipeline;
mod render;
mod render_loop;
mod render_pass;
mod shader;
mod vertex;

crate use device::*;
crate use format::*;
crate use framebuffer::*;
crate use global::*;
crate use image::*;
crate use pipeline::*;
#[allow(unused_imports)]
crate use render::*;
crate use render_loop::*;
crate use render_pass::*;
crate use shader::*;
crate use vertex::*;

mod config;

pub use config::*;

unit::collect_tests![
    device,
    format,
    //framebuffer,
    global,
    image,
    pipeline,
    render_pass,
    vertex,
];

#[cfg(test)]
mod testing;

#[cfg(test)]
fn main() {
    testing::run_tests();
}
