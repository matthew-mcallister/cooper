#![allow(unused_imports)]

mod debug;
mod descriptor;
mod device;
mod framebuffer;
mod instance;
mod memory;
mod render_pass;
mod shader;
mod swapchain;

crate use debug::*;
crate use descriptor::*;
crate use device::*;
crate use framebuffer::*;
pub use instance::*;
crate use memory::*;
crate use render_pass::*;
crate use shader::*;
crate use swapchain::*;

unit::collect_tests![
    descriptor,
    instance,
    memory,
];
