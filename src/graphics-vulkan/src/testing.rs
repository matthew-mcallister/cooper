//! This module provides an environment for unit testing graphics engine
//! code.
//!
//! # Features
//!
//! - Bails out on testing if there is an error initializing Vulkan
//! - Requires tests to pass validation layers

#![cfg(test)]

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
    crate swapchain: Arc<Swapchain>,
    crate queues: Vec<Vec<Arc<Queue>>>,
}

impl TestVars {
    crate fn device(&self) -> &Arc<Device> {
        &self.swapchain.device
    }
}

const WINDOW_DIMS: (i32, i32) = (320, 200);

impl VulkanTestContext {
    unsafe fn init_vars(&self) -> Result<TestVars, AnyError> {
        const NAME: &'static str = "cooper unit test";
        let info = window::CreateInfo {
            title: NAME.to_owned(),
            dims: WINDOW_DIMS.into(),
            hints: window::CreationHints {
                hidden: true,
                ..Default::default()
            },
        };
        let window = Arc::new(self.proxy.create_window(info)?);
        let app = AppInfo {
            name: NAME.to_owned(),
            version: [0, 1, 0],
            debug: true,
            ..Default::default()
        };

        let instance =
            Arc::new(Instance::new(window.vk_platform().clone(), info)?);
        let surface = instance.create_surface(&window)?;
        let pdev = device_for_surface(&surface)?;
        let (device, queues) = instance.create_device(pdev)?;
        let swapchain = device.create_swapchain(&surface)?;
        Ok(TestVars {
            swapchain,
            queues,
        })
    }
}

impl unit::PanicTestInvoker<VulkanTestData> for VulkanTestContext {
    fn invoke(&self, test: &unit::Test<VulkanTestData>) {
        // Recreate the full state so that every test has a clean slate.
        unsafe {
            let vars = self.init_vars().unwrap_or_else(|e| {
                panic!("failed to initialize video: {}", e);
            });

            // TODO: Today, just run the test and see that it doesn't
            // crash. Tomorrow, mark the test as failed if the
            // validation layer reports any errors or warnings.
            (test.data())(vars);
        }
    }
}

crate fn run_tests() {
    let (mut evt, proxy) = unsafe { window::init().unwrap() };
    let thread = thread::spawn(move || {
        let context = VulkanTestContext::new(proxy);
        let context = unit::PanicTestContext::new(context);
        let mut builder = unit::TestDriverBuilder::<VulkanTest>::new();
        crate::__collect_tests(&mut builder);
        let mut driver = builder.build(Box::new(context));
        driver.run();
    });
    evt.pump();
    thread.join().unwrap();
}
