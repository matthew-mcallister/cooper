use std::thread;

use crate::*;

pub type CollectFn<D> = fn(&mut unit::TestDriverBuilder<unit::Test<D>>);

/// A test runner that supplies each test invocation with a proxy to an
/// event loop running on the main thread. Necessary to run tests across
/// multiple platforms.
pub fn run_tests<C, D>(collect: CollectFn<D>)
where
    C: unit::PanicTestInvoker<D> + From<EventLoopProxy> + 'static,
    D: std::panic::RefUnwindSafe + 'static,
{
    let (mut evt, proxy) = unsafe { init().unwrap() };

    let builder = thread::Builder::new().name("test_0".into());
    builder.spawn(move || {
        let context: C = proxy.into();
        let context = unit::PanicTestContext::new(context);
        let mut builder = unit::TestDriverBuilder::<Test<D>>::parse_args();
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
