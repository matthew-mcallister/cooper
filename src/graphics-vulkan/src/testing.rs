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

pub type VulkanTestData = unsafe fn(Arc<Swapchain>);
pub type VulkanTest = unit::Test<VulkanTestData>;

#[derive(Constructor, Debug)]
pub struct VulkanTestContext {
    proxy: window::EventLoopProxy,
}

const WINDOW_DIMS: (i32, i32) = (320, 200);

impl VulkanTestContext {
    unsafe fn init_swapchain(&self) -> Result<Arc<Swapchain>, AnyError> {
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
        let config = Arc::new(GraphicsConfig {
            app_name: NAME.to_owned(),
            app_version: [0, 1, 0],
            debug: true,
            ..Default::default()
        });

        let instance =
            Arc::new(Instance::new(window.vk_platform().clone(), config)?);
        let surface = instance.create_surface(&window)?;
        let pdev = device_for_surface(&surface)?;
        let device = instance.create_device(pdev)?;
        Ok(device.create_swapchain(&surface)?)
    }
}

impl unit::PanicInvocationHelper<VulkanTestData> for VulkanTestContext {
    fn invoke(&self, f: &VulkanTestData) {
        // Recreate the full state so that every test has a clean slate.
        unsafe {
            let swapchain = self.init_swapchain().unwrap_or_else(|e| {
                panic!("failed to initialize video: {}", e);
            });

            // TODO: Today, just run the test and see that it doesn't
            // crash. Tomorrow, mark the test as failed if the
            // validation layer reports any errors or warnings.
            f(swapchain);
        }
    }
}

pub fn run_tests() {
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
