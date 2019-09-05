#![feature(set_stdio)]

use enum_map::Enum;

mod context;
#[macro_use]
mod macros;
mod reporter;

pub use context::*;
pub use macros::*;
pub use reporter::*;

/// Provides the environment in which tests are run.
pub trait TestContext<T>: std::fmt::Debug {
    fn run(&mut self, test: &T) -> Result<(), Option<String>>;
}

/// The interpretation of the results of an executed test.
#[derive(Clone, Copy, Debug, Enum, Eq, PartialEq)]
pub enum Outcome {
    Passed,
    Failed,
    Xpassed,
    Xfailed,
    Ignored,
    Filtered,
}

impl Outcome {
    fn is_critical(&self) -> bool {
        [Outcome::Failed, Outcome::Xpassed].contains(self)
    }
}

/// The output from a test.
#[derive(Clone, Debug)]
pub struct TestResult {
    outcome: Outcome,
    output: Option<String>,
}

/// Exports or displays test results.
pub trait TestReporter<T>: std::fmt::Debug {
    /// Called at the beginning of testing.
    fn before_all(&mut self, tests: &[T]);

    /// Called in real time after each test is started.
    fn before_each(&mut self, test: &T);

    /// Called in real time after each test is completed.
    fn after_each(&mut self, test: &T, result: &TestResult);

    /// Called once all tests are finished.
    fn after_all(&mut self, tests: &[T], results: &[TestResult]);
}

// TODO: should_panic
#[derive(Clone, Debug, Default)]
pub struct TestAttrs {
    ignore: bool,
    xfail: bool,
}

impl TestAttrs {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn ignore(mut self) -> Self {
        self.ignore = true;
        self
    }

    pub fn xfail(mut self) -> Self {
        self.xfail = true;
        self
    }

    pub fn build_test<D>(self, name: String, data: D) -> Test<D> {
        Test {
            name,
            attrs: self,
            data,
        }
    }
}

/// The "base" test type used by the driver.
// TODO: Interface should be a trait
#[derive(Clone, Debug)]
pub struct Test<D> {
    name: String,
    attrs: TestAttrs,
    data: D,
}

impl<D> Test<D> {
    pub fn name(&self) -> &str {
        &self.name[..]
    }

    pub fn ignore(&self) -> bool {
        self.attrs.ignore
    }

    pub fn xfail(&self) -> bool {
        self.attrs.xfail
    }

    pub fn data(&self) -> &D {
        &self.data
    }
}

/// Collects tests for execution and allows configuring how tests are
/// processed.
// TODO: Filters
// TODO: Optionally run tests in parallel
#[derive(Debug, Default)]
pub struct TestDriverBuilder<T> {
    tests: Vec<T>,
    reporter: Option<Box<dyn TestReporter<T>>>,
}

impl<T> TestDriverBuilder<T> {
    pub fn new() -> Self {
        TestDriverBuilder {
            tests: Vec::new(),
            reporter: None,
        }
    }

    pub fn add_test(&mut self, test: T) -> &mut Self {
        self.tests.push(test);
        self
    }

    pub fn add_tests(&mut self, tests: impl IntoIterator<Item = T>) ->
        &mut Self
    {
        self.tests.extend(tests);
        self
    }

    pub fn set_reporter(&mut self, reporter: Box<dyn TestReporter<T>>) ->
        &mut Self
    {
        self.reporter = Some(reporter);
        self
    }
}

impl<D> TestDriverBuilder<Test<D>> {
    pub fn build(self, context: Box<dyn TestContext<Test<D>>>) ->
        TestDriver<D>
    {
        let reporter = self.reporter
            .unwrap_or_else(|| Box::new(StandardTestReporter::stdout()));
        TestDriver {
            tests: self.tests,
            results: Vec::new(),
            reporter,
            context,
        }
    }
}

/// Executes tests and reports results.
#[derive(Debug)]
pub struct TestDriver<D> {
    tests: Vec<Test<D>>,
    results: Vec<TestResult>,
    reporter: Box<dyn TestReporter<Test<D>>>,
    context: Box<dyn TestContext<Test<D>>>,
}

impl<D> TestDriver<D> {
    pub fn run(&mut self) {
        self.reporter.before_all(&self.tests);
        for test in self.tests.iter() {
            self.reporter.before_each(test);

            let (outcome, output);
            if test.ignore() {
                outcome = Outcome::Ignored;
                output = None;
            } else {
                let (on_pass, on_fail) =
                    if test.xfail() { (Outcome::Xpassed, Outcome::Xfailed) }
                    else { (Outcome::Passed, Outcome::Failed) };
                match self.context.run(test) {
                    Ok(()) => {
                        outcome = on_pass;
                        output = None;
                    }
                    Err(m) => {
                        outcome = on_fail;
                        output = m;
                    },
                }
            }
            let result = TestResult { outcome, output };

            self.reporter.after_each(test, &result);
            self.results.push(result);
        }
        self.reporter.after_all(&self.tests[..], &self.results[..]);
    }
}
