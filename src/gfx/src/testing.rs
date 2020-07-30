use std::os::raw::c_int;
use std::sync::Arc;
use std::thread;

use derive_more::From;
use prelude::*;

use crate::*;

// TODO: This really *shouldn't* be public, but it basically has to be
// for integration tests.
pub type TestInput = Box<RenderLoop>;
pub type TestData = unsafe fn(TestInput);
pub type Test = unit::Test<TestData>;

#[derive(Debug, From)]
pub struct TestContext {
    proxy: window::EventLoopProxy,
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

    unsafe fn create_render_loop(&self) -> Result<Box<RenderLoop>, AnyError> {
        Ok(Box::new(RenderLoop::new(app_info(), self.create_window()?)?))
    }
}

impl unit::PanicTestInvoker<TestData> for TestContext {
    fn invoke(&self, test: &IntegrationTest) {
        self.proxy.poke();
        let vars = unsafe { self.create_render_loop() }.unwrap_or_else(|e| {
            panic!("failed to initialize: {}", e);
        });
        unsafe { (test.data())(vars); }
    }
}
