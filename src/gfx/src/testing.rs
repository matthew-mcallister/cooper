//! This module provides an environment for unit testing graphics engine
//! code.
//!
//! # Features
//!
//! - Bails out on testing if there is an error initializing Vulkan
//! - Requires tests to pass validation layers

use std::os::raw::c_int;
use std::sync::Arc;
use std::thread;

use prelude::*;

use crate::*;

pub type TestData<T> = unsafe fn(T);
pub type Test<T> = unit::Test<TestData<T>>;

crate type UnitTestInput = TestVars;
crate type UnitTest = Test<UnitTestInput>;

pub type IntegrationTestInput = Box<RenderLoop>;
pub type IntegrationTest = Test<IntegrationTestInput>;

#[derive(Debug)]
pub struct TestContext {
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

const WINDOW_NAME: &'static str = "cooper test";
const WINDOW_DIMS: (u32, u32) = (1920, 1080);

fn app_info() -> AppInfo {
    AppInfo {
        name: WINDOW_NAME.to_owned(),
        version: [0, 1, 0],
        debug: true,
        test: true,
        ..Default::default()
    }
}

impl TestContext {
    fn create_window(&self) -> Result<Arc<window::Window>, AnyError> {
        let show_window = std::env::var("TESTING_SHOW_WINDOW")
            .map_or(false, |val| val == "1");

        let info = window::CreateInfo {
            title: WINDOW_NAME.to_owned(),
            dims: (WINDOW_DIMS.0 as c_int, WINDOW_DIMS.1 as c_int).into(),
            hints: window::CreationHints {
                hidden: !show_window,
                ..Default::default()
            },
        };
        Ok(Arc::new(self.proxy.create_window(info)?))
    }

    unsafe fn create_swapchain(&self) -> Result<TestVars, AnyError> {
        let window = self.create_window()?;
        let app_info = app_info();
        let vk_platform = window.vk_platform().clone();
        let instance = Arc::new(Instance::new(vk_platform, app_info)?);
        let surface = instance.create_surface(&window)?;
        let pdev = device_for_surface(&surface)?;
        let (device, queues) = instance.create_device(pdev)?;
        let swapchain = device.create_swapchain(surface)?;
        let gfx_queue = Arc::clone(&queues[0][0]);
        Ok(TestVars {
            swapchain,
            gfx_queue,
        })
    }

    unsafe fn create_render_loop(&self) -> Result<Box<RenderLoop>, AnyError> {
        Ok(Box::new(RenderLoop::new(app_info(), self.create_window()?)?))
    }
}

impl unit::PanicTestInvoker<TestData<UnitTestInput>> for TestContext {
    fn invoke(&self, test: &UnitTest) {
        self.proxy.poke();  // Refresh timeout
        let vars = unsafe { self.create_swapchain() }.unwrap_or_else(|e| {
            panic!("failed to initialize: {}", e);
        });
        unsafe { (test.data())(vars); }
    }
}

impl unit::PanicTestInvoker<TestData<IntegrationTestInput>> for TestContext {
    fn invoke(&self, test: &IntegrationTest) {
        self.proxy.poke();
        let vars = unsafe { self.create_render_loop() }.unwrap_or_else(|e| {
            panic!("failed to initialize: {}", e);
        });
        unsafe { (test.data())(vars); }
    }
}

pub fn run_tests<T>(collect: fn(&mut unit::TestDriverBuilder<Test<T>>))
where
    T: 'static,
    TestContext: unit::PanicTestInvoker<TestData<T>>,
{
    let (mut evt, proxy) = unsafe { window::init().unwrap() };

    let builder = thread::Builder::new().name("test_0".into());
    builder.spawn(move || {
        let context = TestContext { proxy };
        let context = unit::PanicTestContext::new(context);
        let mut builder = unit::TestDriverBuilder::<Test<T>>::parse_args();
        collect(&mut builder);
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
