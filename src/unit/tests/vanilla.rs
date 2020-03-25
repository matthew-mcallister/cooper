use cooper_unit::*;

mod tests;

macro_rules! test {
    ($(@$attr:ident)* $fn:path) => {{
        let name = concat!(module_path!(), "::", stringify!($fn)).to_owned();
        TestAttrs::new()
            $(.$attr())*
            .build_test(name, $fn)
    }}
}

fn add_tests(builder: &mut TestDriverBuilder<PlainTest>) {
    builder
        .add_test(test!(tests::vanilla::test_disjoint))
        .add_test(test!(tests::vanilla::test_subset_and_superset))
        .add_test(test!(tests::vanilla::test_iterate))
        .add_test(test!(@ignore tests::vanilla::test_ignore))
        .add_test(test!(@xfail tests::vanilla::test_xfail))
        .add_test(test!(@ignore @xfail tests::vanilla::test_ignore_xfail))
        .add_test(test!(@should_err tests::vanilla::test_should_err))
        // These two tests actually fail
        .add_test(test!(tests::vanilla::test_failure))
        .add_test(test!(@xfail tests::vanilla::test_xpass));
}

fn main() {
    use cooper_unit::*;
    let mut builder = TestDriverBuilder::new();
    add_tests(&mut builder);
    let mut driver = builder.build_basic();
    driver.run();
}
