#![allow(clippy::or_fun_call)]

mod commands;
mod debug;
mod descriptor;
mod device;
mod framebuffer;
mod image;
mod instance;
mod memory;
mod pipeline;
mod queue;
mod render_pass;
mod sampler;
mod shader;
mod swapchain;
mod sync;
mod vertex;

crate use commands::*;
crate use debug::*;
crate use descriptor::*;
crate use device::*;
crate use framebuffer::*;
pub use image::*;
crate use instance::*;
pub use memory::*;
crate use pipeline::*;
crate use queue::*;
crate use render_pass::*;
pub use sampler::*;
crate use shader::*;
crate use swapchain::*;
crate use sync::*;
pub use vertex::*;

#[cfg(test)]
mod tests {
    fn smoke_test(_vars: crate::testing::TestVars) {
        // Do nothing
    }

    fn validation_error_test(vars: crate::testing::TestVars) {
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
    framebuffer,
    image,
    memory,
    pipeline,
    render_pass,
    sampler,
    sync,
    swapchain,
    tests,
    vertex,
];
