mod commands;
mod debug;
mod descriptor;
mod device;
mod instance;
mod memory;
mod swapchain;
mod sync;

crate use commands::*;
crate use debug::*;
crate use descriptor::*;
crate use device::*;
pub use instance::*;
crate use memory::*;
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
    memory,
    swapchain,
    tests,
];
