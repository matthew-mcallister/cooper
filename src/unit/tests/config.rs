//! Tests configuration options.
use cooper_unit::*;

fn test_passing_with_print() {
    println!("I'm a little teapot");
}

fn test_failing_with_print() {
    panic!("Oh no!");
}

macro_rules! test {
    ($(@$attr:ident)* $fn:ident) => {{
        let name = concat!(module_path!(), "::", stringify!($fn)).to_owned();
        TestAttrs::new().build_test(name, $fn)
    }}
}

fn add_tests(builder: &mut TestDriverBuilder<PlainTest>) {
    builder
        .add_test(test!(test_passing_with_print))
        .add_test(test!(test_failing_with_print));
}

fn disable_capture_test() {
    use cooper_unit::*;
    let mut builder = TestDriverBuilder::new();
    builder.set_config(RunnerConfig { disable_capture: true });
    add_tests(&mut builder);
    let mut driver = builder.build_basic();
    driver.run();
}

fn main() {
    disable_capture_test();
}
