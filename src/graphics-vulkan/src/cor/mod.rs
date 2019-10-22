mod config;
mod debug;
mod descriptor;
mod device;
mod framebuffer;
mod image;
mod init;
mod memory;
mod pipeline;
mod render_pass;
mod shader;
mod staging;
mod swapchain;
mod xfer;

crate use config::*;
crate use debug::*;
crate use descriptor::*;
crate use device::*;
crate use framebuffer::*;
crate use image::*;
crate use init::*;
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
    staging,
    xfer,
];
