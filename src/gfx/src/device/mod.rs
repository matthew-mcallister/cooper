mod commands;
mod debug;
mod descriptor;
mod device;
mod framebuffer;
mod image;
mod instance;
mod memory;
mod pipeline;
mod render_pass;
mod sampler;
mod swapchain;
mod sync;

crate use commands::*;
crate use debug::*;
crate use descriptor::*;
crate use device::*;
crate use framebuffer::*;
crate use image::*;
crate use instance::*;
crate use memory::*;
crate use pipeline::*;
crate use render_pass::*;
crate use sampler::*;
crate use swapchain::*;
crate use sync::*;

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
    swapchain,
    tests,
];
