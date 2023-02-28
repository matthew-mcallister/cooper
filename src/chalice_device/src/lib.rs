#![feature(
    arbitrary_self_types,
    cow_is_borrowed,
    entry_insert,
    hash_raw_entry,
    maybe_uninit_slice,
    nonnull_slice_from_raw_parts,
    trait_alias,
    try_blocks,
    type_ascription
)]
#![allow(incomplete_features, path_statements)]
#![allow(
    clippy::missing_safety_doc,
    clippy::module_inception,
    clippy::or_fun_call,
    clippy::too_many_arguments,
    clippy::try_err,
    clippy::type_complexity
)]

macro_rules! err_msg {
    ($msg:literal) => {
        crate::Error(anyhow::anyhow!($msg))
    };
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
mod loader;
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
mod window;

pub use commands::*;
pub use debug::*;
pub use descriptor::*;
pub use device::*;
pub use extent::*;
pub use format::*;
pub use framebuffer::*;
pub use image::*;
pub use instance::*;
pub use loader::*;
pub use memory::*;
pub use pipeline::*;
pub use queue::*;
pub use render_pass::*;
pub use sampler::*;
pub use shader::*;
pub(crate) use staged_cache::*;
pub use swapchain::*;
pub use sync::*;
pub use util::*;
pub use vertex::*;
pub use window::*;

#[cfg(test)]
mod testing;

use derive_more::Display;

#[derive(Debug, Display)]
#[display(fmt = "{}", _0)]
struct StringError(String);

impl std::error::Error for StringError {}

#[derive(Debug, Display)]
#[display(fmt = "{}", _0)]
pub struct Error(anyhow::Error);

impl std::error::Error for Error {}

impl From<vk::Result> for Error {
    fn from(res: vk::Result) -> Self {
        Self(res.into())
    }
}

impl From<String> for Error {
    fn from(msg: String) -> Self {
        Self(anyhow::Error::new(StringError(msg)))
    }
}

impl<'a> From<&'a str> for Error {
    fn from(msg: &'a str) -> Self {
        msg.to_owned().into()
    }
}

// TODO: This should kind of just be std::result::Result<T, vk::Result>
pub type DeviceResult<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use crate::testing::TestVars;

    #[test]
    fn smoke_test() {
        let _ = TestVars::new();
        // Do nothing
    }

    #[test]
    #[should_panic]
    fn validation_error_test() {
        let vars = TestVars::new();
        // Leak a semaphore
        let dt = &*vars.device().table;
        let create_info = vk::SemaphoreCreateInfo::default();
        let mut sem = vk::null();
        unsafe {
            dt.create_semaphore(&create_info, std::ptr::null(), &mut sem)
                .check()
                .unwrap();
        }
    }
}
