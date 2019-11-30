#![allow(unused_imports)]

mod config;
mod debug;
mod descriptor;
mod device;
mod framebuffer;
mod init;
mod memory;
mod render_pass;
mod shader;
mod swapchain;

pub use config::*;
crate use debug::*;
crate use descriptor::*;
crate use device::*;
crate use framebuffer::*;
pub use init::*;
crate use memory::*;
crate use render_pass::*;
crate use shader::*;
crate use swapchain::*;

unit::collect_tests![
    descriptor,
    init,
    memory,
];
