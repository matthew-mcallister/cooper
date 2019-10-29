mod commands;
mod config;
mod debug;
mod descriptor;
mod device;
mod frame;
mod framebuffer;
mod image;
mod init;
mod local;
mod manager;
mod memory;
mod objects;
mod pipeline;
mod render_pass;
mod shader;
mod staging;
mod swapchain;
mod xfer;

crate use commands::*;
pub use config::*;
crate use debug::*;
crate use descriptor::*;
crate use device::*;
crate use frame::*;
crate use framebuffer::*;
crate use image::*;
pub use init::*;
crate use local::*;
crate use manager::*;
crate use memory::*;
crate use pipeline::*;
crate use render_pass::*;
crate use shader::*;
crate use staging::*;
crate use swapchain::*;
crate use xfer::*;

unit::collect_tests![
    descriptor,
    init,
    memory,
    objects,
    staging,
    xfer,
];
