#![feature(
    arbitrary_self_types,
    bool_to_option,
    const_generics,
    cow_is_borrowed,
    crate_visibility_modifier,
    entry_insert,
    hash_raw_entry,
    maybe_uninit_extra,
    maybe_uninit_slice_assume_init,
    or_patterns,
    try_blocks,
    try_trait,
    type_ascription,
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
mod extent;
mod format;
mod frame;
mod global;
mod material;
mod mesh;
mod object;
mod render;
mod resource;
mod rloop;
mod staged_cache;
mod state;
mod world;

pub use device::*;
pub use extent::*;
pub use format::*;
crate use frame::*;
crate use global::*;
pub use material::*;
pub use mesh::*;
pub use object::*;
pub use render::*;
crate use resource::*;
pub use rloop::*;
crate use staged_cache::*;
crate use state::*;
pub use world::*;

unit::collect_tests![
    device,
    extent,
    format,
    global,
    mesh,
    render,
    resource,
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
