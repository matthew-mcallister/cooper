//! This module provides an environment for unit testing graphics engine
//! code.
//!
//! # Features
//!
//! - Bails out on testing if there is an error initializing Vulkan
//! - Requires tests to pass validation layers

#![cfg(test)]

use std::os::raw::c_int;
use std::sync::Arc;
use std::thread;

use derive_more::*;
use prelude::*;

use crate::*;

crate type VulkanTestData = unsafe fn(TestVars);
crate type VulkanTest = unit::Test<VulkanTestData>;

#[derive(Constructor, Debug)]
crate struct VulkanTestContext {
    proxy: window::EventLoopProxy,
}

#[derive(Debug)]
crate struct TestVars {
    crate swapchain: Swapchain,
    crate gfx_queue: Arc<Queue>,
}

impl TestVars {
    crate fn swapchain(&self) -> &Swapchain {
        &self.swapchain
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.swapchain.device
    }

    crate fn gfx_queue(&self) -> &Arc<Queue> {
        &self.gfx_queue
    }
}

const WINDOW_DIMS: (u32, u32) = (1920, 1080);

impl VulkanTestContext {
    // TODO: Only helpful in the device module
    unsafe fn init_vars(&self) -> Result<TestVars, AnyError> {
        const NAME: &'static str = "cooper unit test";
        let info = window::CreateInfo {
            title: NAME.to_owned(),
            dims: (WINDOW_DIMS.0 as c_int, WINDOW_DIMS.1 as c_int).into(),
            hints: window::CreationHints {
                hidden: true,
                ..Default::default()
            },
        };
        let window = Arc::new(self.proxy.create_window(info)?);

        let app_info = AppInfo {
            name: NAME.to_owned(),
            version: [0, 1, 0],
            debug: true,
            test: true,
            ..Default::default()
        };
        let vk_platform = window.vk_platform().clone();
        let instance = Arc::new(Instance::new(vk_platform, app_info)?);
        let surface = instance.create_surface(&window)?;
        let pdev = device_for_surface(&surface)?;
        let (device, queues) = instance.create_device(pdev)?;
        let swapchain = device.create_swapchain(&surface)?;
        let gfx_queue = Arc::clone(&queues[0][0]);
        Ok(TestVars {
            swapchain,
            gfx_queue,
        })
    }
}

impl unit::PanicTestInvoker<VulkanTestData> for VulkanTestContext {
    fn invoke(&self, test: &unit::Test<VulkanTestData>) {
        // Recreate the full state so that every test has a clean slate.
        unsafe {
            // Poke the event loop to refresh the timeout.
            self.proxy.poke();

            let vars = self.init_vars().unwrap_or_else(|e| {
                panic!("failed to initialize video: {}", e);
            });
            let instance = Arc::clone(&vars.swapchain.device.instance);

            (test.data())(vars);

            let msg_count = instance.debug_message_count();
            assert!(msg_count == 0, "caught {} validation errors", msg_count);
        }
    }
}

crate fn run_tests() {
    let (mut evt, proxy) = unsafe { window::init().unwrap() };

    let builder = thread::Builder::new().name("test_0".into());
    builder.spawn(move || {
        let context = VulkanTestContext::new(proxy);
        let context = unit::PanicTestContext::new(context);
        let mut builder = unit::TestDriverBuilder::<VulkanTest>::parse_args();
        crate::__collect_tests(&mut builder);
        let mut driver = builder.build(Box::new(context));
        driver.run();
    }).unwrap();

    evt.set_poll_interval(std::time::Duration::new(1, 0));
    loop {
        match evt.pump_with_timeout() {
            Ok(0) => {
                // When the test thread aborts without unwinding, the
                // channel never gets closed; hence the timeout.
                eprintln!();
                eprintln!("test thread timed out to avert deadlock");
                break;
            },
            Ok(_) => continue,
            Err(_) => break,
        }
    }
}
