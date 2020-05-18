#![feature(
    arbitrary_self_types,
    bool_to_option,
    const_generics,
    cow_is_borrowed,
    crate_visibility_modifier,
    maybe_uninit_extra,
    try_blocks,
    type_ascription,
    weak_into_raw,
)]

#![allow(dead_code, incomplete_features)]

#![allow(
    clippy::missing_safety_doc,
    clippy::module_inception,
    clippy::needless_range_loop,
    clippy::too_many_arguments,
    clippy::try_err,
    clippy::type_complexity,
)]

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
mod material;
mod mesh;
mod render;
mod rloop;
mod scheduler;
mod staged_cache;
mod world;

pub use device::*;
pub use format::*;
crate use global::*;
pub use material::*;
pub use mesh::*;
pub use render::*;
pub use rloop::*;
crate use scheduler::*;
crate use staged_cache::*;
pub use world::*;

unit::collect_tests![
    device,
    format,
    global,
    mesh,
    render,
    scheduler,
    staged_cache,
    world,
];

#[derive(Clone, Debug, Default)]
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
    env_logger::init();
    testing::run_tests();
}
