// TODO: Would be great to work around the mandatory test_type! macro.
#[macro_export]
macro_rules! declare_tests {
    (@entry($builder:expr, $fn:ident)) => {
        $crate::declare_tests!(@entry($builder, ($fn)));
    };
    (@entry($builder:expr, ($(#[$attr:ident])* $fn:ident))) => {
        let name = $crate::declare_tests!(@name($fn));
        let test = $crate::TestAttrs::new()
            $(.$attr())*
            .build_test(name, $fn as _);
        $builder.add_test(test);
    };
    (@name($fn:ident)) => {
        concat!(module_path!(), "::", stringify!($fn)).to_owned()
    };
    ($($entry:tt),*$(,)*) => {
        #[cfg(test)]
        pub(crate) fn __collect_tests
            (builder: &mut $crate::TestDriverBuilder<test_type!()>)
        {
            $($crate::declare_tests!(@entry(builder, $entry));)*
        }
    };
}

#[macro_export]
macro_rules! collect_tests {
    ($($($seg:ident)::+),*$(,)*) => {
        #[cfg(test)]
        pub(crate) fn __collect_tests
            (builder: &mut $crate::TestDriverBuilder<test_type!()>)
        {
            $($($seg::)*__collect_tests(builder);)*
        }
    }
}
