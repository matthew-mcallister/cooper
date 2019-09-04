use std::io;
use std::sync::{Arc, Mutex};

use derive_more::*;

use crate::*;

/// Test type used by `PlainTestContext`.
pub type PlainTest = Test<fn()>;

/// Enables running vanilla Rust unit tests. Each test is a function
/// which takes no input and produces no output (except through
/// side-effects). Test failure is signaled by panicking.
#[derive(Debug)]
pub struct PlainTestContext {
    _priv: (),
}

impl PlainTestContext {
    pub fn new() -> Self {
        PlainTestContext { _priv: () }
    }
}

/// Allows writing to a shared buffer as a byte stream.
#[derive(Clone, Constructor, Debug)]
struct Sink {
    inner: Arc<Mutex<Vec<u8>>>,
}

impl io::Write for Sink {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        io::Write::write(&mut *self.inner.lock().unwrap(), data)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl TestContext<PlainTest> for PlainTestContext {
    fn run(&mut self, test: &PlainTest) -> Result<(), Option<String>> {
        // Capture print macro calls to this buffer
        // TODO: see https://github.com/rust-lang/rust/issues/12309
        let output = Sink::new(Arc::new(Mutex::new(Vec::<u8>::new())));
        let (old_stdout, old_stderr) = (
            std::io::set_print(Some(Box::new(Sink::clone(&output)))),
            std::io::set_panic(Some(Box::new(Sink::clone(&output)))),
        );

        let res = std::panic::catch_unwind(test.data);

        std::io::set_print(old_stdout);
        std::io::set_panic(old_stderr);

        match res {
            Ok(_) => Ok(()),
            Err(_) => {
                let bytes = Arc::try_unwrap(output.inner).unwrap()
                    .into_inner().unwrap();
                Err(String::from_utf8(bytes).ok().filter(|s| !s.is_empty()))
            },
        }
    }
}
