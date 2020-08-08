#![feature(
    arbitrary_self_types,
    bool_to_option,
    const_fn,
    const_generics,
    const_panic,
    const_raw_ptr_deref,
    const_raw_ptr_to_usize_cast,
    const_slice_from_raw_parts,
    cow_is_borrowed,
    crate_visibility_modifier,
    entry_insert,
    hash_raw_entry,
    maybe_uninit_extra,
    maybe_uninit_ref,
    maybe_uninit_slice_assume_init,
    or_patterns,
    trait_alias,
    try_blocks,
    try_trait,
    type_ascription,
)]

#![allow(incomplete_features)]

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
    () => { crate::testing::UnitTest }
}

#[macro_use]
mod util;

mod global;
mod material;
mod mesh;
mod object;
mod render;
mod resource;
mod rloop;
mod shader;
mod state;
mod world;

crate use global::*;
pub use material::*;
pub use mesh::*;
pub use object::*;
pub use render::*;
pub use resource::*;
pub use rloop::*;
pub use shader::*;
crate use state::*;
pub use world::*;

// TODO: Put enums into a module device::enums for a glob re-export.
pub use device::{
    AnisotropyLevel, AppInfo, Extent2D, Extent3D, Filter, Format, ImageDef,
    ImageType, IndexType, Lifetime, SamplerAddressMode, SamplerDesc,
    SamplerMipmapMode, VertexAttr,
};

unit::collect_tests![
    resource,
    world,
];

// TODO: This module should really, really not be public, but it has to
// be for integration tests. Waiting for a solution from Cargo.
pub mod testing;

#[cfg(test)]
fn main() {
    env_logger::init();
    window::testing::run_tests::<testing::TestContext, _>(__collect_tests);
}
