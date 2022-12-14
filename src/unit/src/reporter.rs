use std::io;
use std::io::Write;
use std::time;

use enum_map::EnumMap;

use crate::*;

macro_rules! write {
    ($($args:tt)*) => {
        std::write!($($args)*).unwrap()
    }
}

macro_rules! writeln {
    ($($args:tt)*) => {
        std::writeln!($($args)*).unwrap()
    }
}

#[derive(Debug)]
struct Summary {
    start_time: time::Instant,
    end_time: time::Instant,
    total: usize,
    counts: EnumMap<Outcome, usize>,
    critical: Vec<usize>,
}

impl Summary {
    fn new() -> Self {
        let cur_time = time::Instant::now();
        Summary {
            start_time: cur_time,
            end_time: cur_time,
            total: 0,
            counts: Default::default(),
            critical: Default::default(),
        }
    }

    fn begin(&mut self) {
        self.start_time = time::Instant::now();
    }

    fn add_test<D>(
        &mut self,
        _test: &Test<D>,
        result: &TestResult,
    ) {
        self.counts[result.outcome] += 1;
        if result.outcome.is_critical() {
            self.critical.push(self.total);
        }
        self.total += 1;
    }

    fn end(&mut self) {
        self.end_time = time::Instant::now();
    }

    fn elapsed_sec(&self) -> f64 {
        let millis = (self.end_time - self.start_time).as_millis();
        millis as f64 * 1e-3
    }
}

// TODO: Color
#[derive(Debug)]
pub struct StandardTestReporter<W: io::Write + std::fmt::Debug> {
    out: W,
    summary: Summary,
    name_width: usize,
    config: RunnerConfig,
}

impl StandardTestReporter<io::Stdout> {
    pub fn stdout() -> Self {
        StandardTestReporter::with_output(io::stdout())
    }
}

impl<W: io::Write + std::fmt::Debug> StandardTestReporter<W> {
    pub fn with_output(output: W) -> Self {
        StandardTestReporter {
            out: output,
            summary: Summary::new(),
            name_width: 0,
            config: Default::default(),
        }
    }
}

impl<D, W: io::Write + std::fmt::Debug> TestReporter<Test<D>>
    for StandardTestReporter<W>
{
    fn set_config(&mut self, config: RunnerConfig) {
        self.config = config;
    }

    fn before_all(&mut self, tests: &[Test<D>]) {
        writeln!(self.out);

        self.name_width = tests.iter()
            // FIXME: column width calculation not internationalized
            .map(|test| test.name().len())
            .max()
            .unwrap_or(0);
        writeln!(self.out, "running {} tests", tests.len());
        io::stdout().flush().unwrap();

        self.summary.begin();
    }

    fn before_each(&mut self, test: &Test<D>, filter_matches: bool) {
        if !filter_matches { return; }
        write!(
            self.out,
            "test {:width$} ... ",
            test.name(),
            width = self.name_width,
        );
        io::stdout().flush().unwrap();
    }

    fn after_each(&mut self, test: &Test<D>, result: &TestResult) {
        self.summary.add_test(test, result);

        let s = match result.outcome {
            Outcome::Passed => "ok",
            Outcome::Failed => "FAILED",
            Outcome::Xpassed => "XPASSED",
            Outcome::Xfailed => "xfailed",
            Outcome::Ignored => "ignored",
            // TODO: Option to show filtered tests
            Outcome::Filtered => return,
        };
        if self.config.disable_capture {
            // Make it easier to see the outcome through all the output
            writeln!(self.out, ">>> {} <<<", s);
        } else {
            writeln!(self.out, "{}", s);
        }
        io::stdout().flush().unwrap();
    }

    fn after_all(&mut self, tests: &[Test<D>], results: &[TestResult]) {
        self.summary.end();

        writeln!(self.out);
        if !self.summary.critical.is_empty() {
            let critical = self.summary.critical.iter()
                .map(|&i| (&tests[i], &results[i]));

            // Print error messages
            writeln!(self.out, "failures:");
            writeln!(self.out);
            for (test, res) in critical.clone() {
                writeln!(self.out, "---- {} ----", test.name());
                if let Some(ref msg) = res.output {
                    writeln!(self.out, "{}", msg);
                } else if res.outcome == Outcome::Xpassed {
                    writeln!(self.out, "test changed from failing to passing");
                } else if test.should_err() {
                    writeln!(self.out, "test failed to raise an error");
                }
            }
            writeln!(self.out);

            // List failing tests
            writeln!(self.out, "failures:");
            writeln!(self.out);
            for (test, _res) in critical.clone() {
                writeln!(self.out, "    {}", test.name());
            }
            writeln!(self.out);
        }

        // Write summary
        let sum_str =
            if self.summary.critical.is_empty() { "ok" } else { "FAILED" };
        writeln!(self.out, "test result: {}", sum_str);

        writeln!(
            self.out,
            "finished {} tests in {:.3}s",
            self.summary.total,
            self.summary.elapsed_sec(),
        );

        let pairs = [
            ("passed", Outcome::Passed),
            ("failed", Outcome::Failed),
            ("xpassed", Outcome::Xpassed),
            ("xfailed", Outcome::Xfailed),
            ("ignored", Outcome::Ignored),
            ("filtered", Outcome::Filtered),
        ];
        for &(name, outcome) in pairs.iter() {
            let count = self.summary.counts[outcome];
            write!(self.out, "{} {}; ", count, name);
        }
        writeln!(self.out);
        writeln!(self.out);
        io::stdout().flush().unwrap();
    }
}
