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
    maybe_uninit_slice,
    nonnull_slice_from_raw_parts,
    or_patterns,
    trait_alias,
    try_blocks,
    try_trait,
    type_ascription,
)]

#![allow(
    incomplete_features,
    path_statements,
)]
#![allow(
    clippy::missing_safety_doc,
    clippy::module_inception,
    clippy::or_fun_call,
    clippy::too_many_arguments,
    clippy::try_err,
    clippy::type_complexity,
)]

#[cfg(test)]
macro_rules! test_type {
    () => { crate::testing::Test }
}

macro_rules! err_msg {
    ($msg:literal) => {
        crate::Error(anyhow::anyhow!($msg))
    }
}

#[macro_use]
mod util;

mod commands;
mod debug;
mod descriptor;
mod device;
mod extent;
mod format;
mod framebuffer;
mod image;
mod instance;
mod memory;
mod pipeline;
mod queue;
mod render_pass;
mod sampler;
mod shader;
mod staged_cache;
mod swapchain;
mod sync;
mod vertex;

pub use commands::*;
pub use debug::*;
pub use descriptor::*;
pub use device::*;
pub use extent::*;
pub use format::*;
pub use framebuffer::*;
pub use image::*;
pub use instance::*;
pub use memory::*;
pub use pipeline::*;
pub use queue::*;
pub use render_pass::*;
pub use sampler::*;
pub use shader::*;
crate use staged_cache::*;
pub use swapchain::*;
pub use sync::*;
pub use util::*;
pub use vertex::*;

#[cfg(test)]
mod testing;

use derive_more::Display;

#[derive(Debug, Display)]
#[display(fmt = "{}", _0)]
pub struct Error(anyhow::Error);

impl std::error::Error for Error {}

impl From<vk::Result> for Error {
    fn from(res: vk::Result) -> Self {
        Self(res.into())
    }
}

pub type DeviceResult<T> = std::result::Result<T, Error>;

#[cfg(test)]
fn main() {
    env_logger::init();
    window::testing::run_tests::<testing::TestContext, _>(__collect_tests);
}

#[cfg(test)]
mod tests {
    use crate::testing::*;

    fn smoke_test(_vars: TestVars) {
        // Do nothing
    }

    fn validation_error_test(vars: TestVars) {
        // Leak a semaphore
        let dt = &*vars.device().table;
        let create_info = vk::SemaphoreCreateInfo::default();
        let mut sem = vk::null();
        unsafe {
            dt.create_semaphore(&create_info, std::ptr::null(), &mut sem)
                .check().unwrap();
        }
    }

    unit::declare_tests![
        smoke_test,
        (#[should_err] validation_error_test),
    ];
}

unit::collect_tests![
    commands,
    descriptor,
    extent,
    format,
    framebuffer,
    image,
    memory,
    pipeline,
    render_pass,
    sampler,
    staged_cache,
    sync,
    swapchain,
    tests,
    vertex,
];
