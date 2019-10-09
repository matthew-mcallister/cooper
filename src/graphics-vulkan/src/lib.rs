// TODO: Small string optimization
#![feature(arbitrary_self_types)]
#![feature(crate_visibility_modifier)]
#![feature(non_exhaustive)]
#![feature(seek_convenience)]
#![feature(try_blocks)]

#[cfg(test)]
macro_rules! test_type {
    () => { crate::testing::VulkanTest }
}

mod debug;
mod descriptor;
mod image;
mod init;
mod memory;
mod pipeline;
mod render_pass;
mod shader;
mod staging;
mod xfer;

pub use debug::*;
pub use descriptor::*;
pub use image::*;
pub use init::*;
pub use memory::*;
pub use pipeline::*;
pub use render_pass::*;
pub use shader::*;
pub use staging::*;
pub use xfer::*;

unit::collect_tests![
    descriptor,
    init,
    memory,
    pipeline,
    render_pass,
    shader,
    staging,
    xfer,
];

#[cfg(test)]
mod testing;

#[cfg(test)]
fn main() {
    testing::run_tests();
}
