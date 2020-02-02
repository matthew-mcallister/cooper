use std::io;
use std::panic::RefUnwindSafe;
use std::sync::{Arc, Mutex};

use derivative::Derivative;
use derive_more::*;

use crate::*;

/// Implementors of this trait are responsible for running individual
/// tests which may be caught panicking. It is automatically implemented
/// for most types that implement `Fn(&D)`. If this type uses internal
/// mutability, it must be marked as `RefUnwindSafe`, as it will be
/// referenced from inside `catch_panic`.
pub trait PanicTestInvoker<D>: RefUnwindSafe + std::fmt::Debug {
    /// Runs the test.
    fn invoke(&self, test: &Test<D>);
}

impl<D, F> PanicTestInvoker<D> for F
    where F: Fn(&D) + RefUnwindSafe + std::fmt::Debug
{
    fn invoke(&self, test: &Test<D>) {
        self(test.data())
    }
}

/// The test type of the vanilla Rust test runner.
pub type PlainTest = Test<fn()>;

#[derive(Constructor, Debug, Default)]
pub struct PlainTestInvoker {}

impl PanicTestInvoker<fn()> for PlainTestInvoker {
    fn invoke(&self, test: &PlainTest) {
        (test.data())()
    }
}

/// Runs tests where failure is signaled by panicking. This type wraps a
/// "test invocation helper", which is at minimum responsible for
/// running the test, but may optionally do things such as
/// setup/teardown or observing the test's side effects.
///
/// In order to use this wrapper on `Test<D>`, `F` must implement two
/// key traits:
///
/// - `PanicTestInvoker`: This impl should actually *run* the test.
///   Notice that this trait borrows immutably---see below.
/// - `RefUnwindSafe`: This marker trait is required to use
///   `std::panic::catch_panic` to recover from a failed test.
///
/// Rust attempts to facilitate writing exception-safe code by limiting
/// the use of side-effects within a `catch_panic` call---thus, the
/// invocation helper cannot be borrowed mutably and must rely on
/// internal mutability for stateful setup/teardown. If taking this
/// route, the second trait constraint may need to be implemented
/// manually.
#[derive(Debug, Default)]
pub struct PanicTestContext<F> {
    inner: F,
    config: RunnerConfig,
}

/// Allows writing to a shared buffer as a byte stream.
#[derive(Clone, Constructor, Debug)]
struct Sink {
    inner: Arc<Mutex<Vec<u8>>>,
}

/// Diverts the `print` and `panic` macros to a buffer.
#[derive(Derivative)]
#[derivative(Debug)]
struct PrintCapture {
    sink: Sink,
    #[derivative(Debug="ignore")]
    old_stdout: Option<Box<dyn io::Write + Send>>,
    #[derivative(Debug="ignore")]
    old_stderr: Option<Box<dyn io::Write + Send>>,
}

impl<F> PanicTestContext<F> {
    pub fn new(inner: F) -> Self {
        PanicTestContext {
            inner,
            config: Default::default()
        }
    }
}

impl<D, F> TestContext<Test<D>> for PanicTestContext<F>
where
    D: std::panic::RefUnwindSafe,
    F: PanicTestInvoker<D>,
{
    fn set_config(&mut self, config: RunnerConfig) {
        self.config = config;
    }

    fn run(&mut self, test: &Test<D>) -> Result<(), Option<String>> {
        // Capture print macro calls to this buffer
        // TODO: see https://github.com/rust-lang/rust/issues/12309
        // TODO: capture output of child threads created by tests
        let capture = (!self.config.disable_capture).then(PrintCapture::new);
        let res = std::panic::catch_unwind(|| self.inner.invoke(test));
        match res {
            Ok(_) => Ok(()),
            Err(_) => {
                let bytes = capture.map_or(Vec::new(), |c| c.extract());
                Err(String::from_utf8(bytes).ok().filter(|s| !s.is_empty()))
            },
        }
    }
}

impl io::Write for Sink {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        io::Write::write(&mut *self.inner.lock().unwrap(), data)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for PrintCapture {
    fn drop(&mut self) {
        self.restore();
    }
}

impl PrintCapture {
    fn new() -> Self {
        let sink = Sink::new(Arc::new(Mutex::new(Vec::<u8>::new())));
        PrintCapture {
            old_stdout: io::set_print(Some(Box::new(sink.clone()))),
            old_stderr: io::set_panic(Some(Box::new(sink.clone()))),
            sink,
        }
    }

    fn restore(&mut self) {
        std::io::set_print(self.old_stdout.take());
        std::io::set_panic(self.old_stderr.take());
    }

    fn extract(mut self) -> Vec<u8> {
        self.restore();
        let sink = unsafe { std::ptr::read(&self.sink) };
        std::mem::forget(self);
        Arc::try_unwrap(sink.inner).unwrap().into_inner().unwrap()
    }
}
