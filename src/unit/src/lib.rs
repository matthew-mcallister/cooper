#![feature(bool_to_option)]
#![feature(set_stdio)]
#![feature(try_blocks)]

use derivative::Derivative;
use enum_map::Enum;
use regex::RegexSet;

mod context;
mod filter;
#[macro_use]
mod macros;
mod reporter;

pub use context::*;
pub use filter::*;
pub use macros::*;
pub use reporter::*;

/// Provides the environment in which tests are run.
pub trait TestContext<T>: std::fmt::Debug {
    /// Configures the context.
    fn set_config(&mut self, config: RunnerConfig);

    /// Runs a single test.
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
    /// Configures the reporter.
    fn set_config(&mut self, config: RunnerConfig);

    /// Called at the beginning of testing.
    fn before_all(&mut self, tests: &[T]);

    /// Called in real time after each test is started.
    fn before_each(&mut self, test: &T, filter_matches: bool);

    /// Called in real time after each test is completed.
    fn after_each(&mut self, test: &T, result: &TestResult);

    /// Called once all tests are finished.
    fn after_all(&mut self, tests: &[T], results: &[TestResult]);
}

#[derive(Clone, Debug, Default)]
pub struct TestAttrs {
    ignore: bool,
    xfail: bool,
    should_err: bool,
}

/// The "base" test type used by the driver.
// TODO: Interface should be a trait
#[derive(Clone, Debug)]
pub struct Test<D> {
    name: String,
    attrs: TestAttrs,
    data: D,
}

impl TestAttrs {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn ignore(self) -> Self {
        TestAttrs {
            ignore: true,
            ..self
        }
    }

    pub fn xfail(self) -> Self {
        TestAttrs {
            xfail: true,
            ..self
        }
    }

    pub fn should_err(self) -> Self {
        TestAttrs {
            should_err: true,
            ..self
        }
    }

    pub fn build_test<D>(self, name: String, data: D) -> Test<D> {
        Test {
            name,
            attrs: self,
            data,
        }
    }
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

    pub fn should_err(&self) -> bool {
        self.attrs.should_err
    }

    pub fn data(&self) -> &D {
        &self.data
    }
}

/// Collects tests for execution and allows configuring how tests are
/// processed.
#[derive(Debug, Derivative)]
#[derivative(Default(bound = ""))]
pub struct TestDriverBuilder<T> {
    tests: Vec<T>,
    reporter: Option<Box<dyn TestReporter<T>>>,
    filter: Option<Box<dyn TestFilter<T>>>,
    config: RunnerConfig,
}

#[derive(Clone, Default, Debug)]
pub struct RunnerConfig {
    pub disable_capture: bool,
}

/// Executes tests and reports results.
#[derive(Debug)]
pub struct TestDriver<D> {
    tests: Vec<Test<D>>,
    results: Vec<TestResult>,
    reporter: Box<dyn TestReporter<Test<D>>>,
    context: Box<dyn TestContext<Test<D>>>,
    filter: Option<Box<dyn TestFilter<Test<D>>>>,
}

impl<T> TestDriverBuilder<T> {
    pub fn new() -> Self {
        Default::default()
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

    pub fn set_filter(&mut self, filter: Box<dyn TestFilter<T>>) -> &mut Self {
        self.filter = Some(filter);
        self
    }

    pub fn set_config(&mut self, config: RunnerConfig) -> &mut Self {
        self.config = config;
        self
    }
}

impl<D> TestDriverBuilder<Test<D>> {
    /// Initializes a test builder by parsing command line args.
    pub fn parse_args() -> Self {
        let args = clap::App::new("test")
            .arg(clap::Arg::with_name("nocapture")
                .long("nocapture")
                .help(concat!(
                    "Inhibits capture of test output. Print statements ",
                    "will show on stdout/stderr.",
                )))
            .arg(clap::Arg::with_name("filter")
                .short("f")
                .long("filter")
                .takes_value(true)
                .multiple(true)
                .help(concat!(
                    "Filters tests by regex matching. Multiple patterns may ",
                    "be provided to match additional tests.",
                )))
            .get_matches();

        let mut builder = Self::new();
        builder.set_config(RunnerConfig {
            disable_capture: args.is_present("nocapture"),
        });
        let _: Option<_> = try {
            let regex = RegexSet::new(args.values_of("filter")?)
                .expect("invalid regex");
            builder.set_filter(Box::new(regex));
        };
        builder
    }

    pub fn build(self, context: Box<dyn TestContext<Test<D>>>) -> TestDriver<D>
    {
        let reporter = self.reporter
            .unwrap_or_else(|| Box::new(StandardTestReporter::stdout()));
        let mut driver = TestDriver {
            tests: self.tests,
            results: Vec::new(),
            reporter,
            context,
            filter: self.filter,
        };
        driver.reporter.set_config(self.config.clone());
        driver.context.set_config(self.config.clone());
        driver
    }
}

impl TestDriverBuilder<PlainTest> {
    pub fn build_basic(self) -> TestDriver<fn()> {
        self.build(Box::new(PanicTestContext::new(PlainTestInvoker::new())))
    }
}

impl<D> TestDriver<D> {
    pub fn run(&mut self) {
        self.reporter.before_all(&self.tests);
        for test in self.tests.iter() {
            let matches = self.filter.is_match(test);

            self.reporter.before_each(test, matches);

            let (outcome, output);
            if !matches {
                outcome = Outcome::Filtered;
                output = None;
            } else if test.ignore() {
                outcome = Outcome::Ignored;
                output = None;
            } else {
                let outcomes = if test.xfail() {
                    [Outcome::Xfailed, Outcome::Xpassed]
                } else {
                    [Outcome::Failed, Outcome::Passed]
                };
                let res = self.context.run(test);
                let passed = res.is_ok() ^ test.should_err();
                outcome = outcomes[passed as usize];
                output = res.err().flatten();
            }
            let result = TestResult { outcome, output };

            self.reporter.after_each(test, &result);
            self.results.push(result);
        }
        self.reporter.after_all(&self.tests[..], &self.results[..]);
    }

    // TODO: fn run_parallel()
}

#[cfg(test)]
fn main() {
}
