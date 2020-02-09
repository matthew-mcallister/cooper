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
mod global;
mod render;
mod scheduler;
mod shader;
mod staged_cache;
mod vertex;

crate use device::*;
crate use format::*;
crate use global::*;
crate use render::*;
crate use scheduler::*;
crate use shader::*;
crate use staged_cache::*;
crate use vertex::*;

mod render_loop;

pub use render_loop::*;

unit::collect_tests![
    device,
    format,
    global,
    render,
    scheduler,
    staged_cache,
    vertex,
];

#[derive(Debug, Default)]
pub struct AppInfo {
    pub name: String,
    pub version: [u32; 3],
    pub debug: bool,
    pub test: bool,
}

#[cfg(test)]
mod testing;

#[cfg(test)]
fn main() {
    testing::run_tests();
}
