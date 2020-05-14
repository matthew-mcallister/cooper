#[macro_use]
extern crate cooper_unit;

macro_rules! test_type {
    () => { cooper_unit::PlainTest }
}

fn mul(a: i32, b: i32) -> i32 {
    a * b
}

mod tests {
    use crate::red::add;
    use super::*;

    fn is_associative() {
        assert_eq!(mul(2, 3), mul(3, 2));
    }

    fn is_distributive() {
        assert_eq!(mul(2, add(3, 4)), add(mul(2, 3), mul(2, 4)));
    }

    #[allow(const_err)]
    #[allow(unconditional_panic)]
    fn divide_by_zero_panics() {
        assert_eq!(1 / 0, 0);
    }

    declare_tests![
        is_associative,
        is_distributive,
        (#[should_err] divide_by_zero_panics),
    ];
}

collect_tests![
    tests,
    red,
    blue,
];

mod red {
    pub fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    mod tests {
        use super::*;

        fn is_commutative() {
            assert_eq!(add(1, 2), add(2, 1));
        }

        fn is_associative() {
            assert_eq!(add(1, add(2, 3)), add(add(1, 2), 3));
        }

        fn is_idempotent() {
            // Might take a while implementing this one...
            assert_eq!(add(1, 1), 1);
        }

        declare_tests![
            is_commutative,
            is_associative,
            (#[xfail] is_idempotent),
        ];
    }

    collect_tests![tests];
}

mod blue {
    pub fn sub(a: i32, b: i32) -> i32 {
        a - b
    }

    mod tests {
        use super::*;

        fn is_anticommutative() {
            assert_eq!(sub(1, 2), -sub(2, 1));
        }

        fn is_commutative() {
            assert_eq!(sub(1, 2), sub(2, 1));
        }

        declare_tests![
            is_anticommutative,
            (#[ignore] is_commutative),
        ];
    }

    collect_tests![tests];
}

fn main() {
    use cooper_unit::*;
    let mut builder = TestDriverBuilder::new();
    crate::__collect_tests(&mut builder);
    let mut driver = builder.build_basic();
    driver.run();
}
