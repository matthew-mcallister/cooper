use std::os::raw::c_int;
use std::sync::Arc;

use derive_more::From;
use device::*;
use prelude::*;

use crate::*;

crate type UnitTestInput = TestVars;
crate type UnitTestData = unsafe fn(UnitTestInput);
crate type UnitTest = unit::Test<UnitTestData>;

// TODO: This really *shouldn't* be public, but it basically has to be
// for integration tests.
pub type IntegrationTestInput = Box<RenderLoop>;
pub type IntegrationTestData = unsafe fn(IntegrationTestInput);
pub type IntegrationTest = unit::Test<IntegrationTestData>;

#[derive(Debug, From)]
pub struct TestContext {
    proxy: window::EventLoopProxy,
}

#[derive(Debug)]
crate struct TestVars {
    crate swapchain: Swapchain,
    crate gfx_queue: Arc<Queue>,
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

impl unit::PanicTestInvoker<UnitTestData> for TestContext {
    fn invoke(&self, test: &UnitTest) {
        let vars = unsafe { self.create_swapchain() }
            .unwrap_or_else(|e| panic!("failed to initialize: {}", e));
        unsafe { (test.data())(vars); }
    }
}

impl unit::PanicTestInvoker<IntegrationTestData> for TestContext {
    fn invoke(&self, test: &IntegrationTest) {
        let vars = unsafe { self.create_render_loop() }
            .unwrap_or_else(|e| panic!("failed to initialize: {}", e));
        unsafe { (test.data())(vars); }
    }
}

#[allow(dead_code)]
impl TestVars {
    crate fn swapchain(&self) -> &Swapchain {
        &self.swapchain
    }

    crate fn device(&self) -> &Arc<Device> {
        self.swapchain.device()
    }

    crate fn gfx_queue(&self) -> &Arc<Queue> {
        &self.gfx_queue
    }
}
