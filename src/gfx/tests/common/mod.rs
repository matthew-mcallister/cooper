/// This is a test harness similar to the gfx::testing module, but
/// modified to fit the needs of integration testing.
// TODO: Merge with gfx::testing

use std::os::raw::c_int;
use std::sync::Arc;
use std::thread;

use cooper_gfx::{AppInfo, RenderLoop};
use derive_more::Constructor;
use prelude::AnyResult;

pub(crate) type TestData = unsafe fn(Box<RenderLoop>);
pub(crate) type Test = unit::Test<TestData>;

const WINDOW_DIMS: (u32, u32) = (1920, 1080);

#[derive(Constructor, Debug)]
pub(crate) struct TestContext {
    proxy: window::EventLoopProxy,
}

impl TestContext {
    unsafe fn test_input(&self) -> AnyResult<Box<RenderLoop>> {
        const NAME: &'static str = "cooper integration test";
        let info = window::CreateInfo {
            title: NAME.to_owned(),
            dims: (WINDOW_DIMS.0 as c_int, WINDOW_DIMS.1 as c_int).into(),
            hints: window::CreationHints {
                hidden: true,
                ..Default::default()
            },
        };
        let window = Arc::new(self.proxy.create_window(info)?);

        let info = AppInfo {
            name: NAME.to_owned(),
            version: [0, 1, 0],
            debug: true,
            test: true,
            ..Default::default()
        };
        Ok(Box::new(RenderLoop::new(info, window)?))
    }
}

impl unit::PanicTestInvoker<TestData> for TestContext {
    fn invoke(&self, test: &unit::Test<TestData>) {
        // Recreate the full state so that every test has a clean slate.
        unsafe {
            // Poke the event loop to refresh the timeout.
            self.proxy.poke();

            let input = self.test_input()
                .unwrap_or_else(|e| panic!("initialization error: {}", e));

            (test.data())(input);
        }
    }
}

pub(crate) fn run_tests() {
    let (mut evt, proxy) = unsafe { window::init().unwrap() };

    let builder = thread::Builder::new().name("test_0".into());
    builder.spawn(move || {
        let context = TestContext::new(proxy);
        let context = unit::PanicTestContext::new(context);
        let mut builder = unit::TestDriverBuilder::<Test>::parse_args();
        crate::__collect_tests(&mut builder);
        let mut driver = builder.build(Box::new(context));
        driver.run();
    }).unwrap();

    evt.set_poll_interval(std::time::Duration::from_secs(1));
    loop {
        match evt.pump_with_timeout() {
            Ok(0) => {
                eprintln!();
                eprintln!("test thread timed out to avert lock-up");
                break;
            },
            Ok(_) => continue,
            Err(_) => break,
        }
    }
}
