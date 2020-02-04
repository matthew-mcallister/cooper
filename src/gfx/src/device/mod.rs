mod commands;
mod debug;
mod descriptor;
mod device;
mod instance;
mod memory;
mod swapchain;

crate use commands::*;
crate use debug::*;
crate use descriptor::*;
crate use device::*;
pub use instance::*;
crate use memory::*;
crate use swapchain::*;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    fn smoke_test(_vars: crate::testing::TestVars) {
        // Do nothing
    }

    fn validation_error_test(vars: crate::testing::TestVars) {
        let device = Arc::clone(&vars.swapchain.device);
        // Leak a semaphore
        unsafe { device.create_semaphore(); }
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
