use cooper_unit::*;

mod tests;

macro_rules! test_type {
    () => { PlainTest }
}

fn test_print_passing() {
    println!("I'm a little teapot");
}

fn test_print_failing() {
    println!("oh no");
    panic!();
}

declare_tests![test_print_passing, test_print_failing];

fn main() {
    let mut builder = TestDriverBuilder::parse_args();
    __collect_tests(&mut builder);
    tests::vanilla::__collect_tests(&mut builder);
    let mut driver = builder.build_basic();
    driver.run();
}
