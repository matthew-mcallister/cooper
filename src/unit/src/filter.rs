// TODO: Reverse filter
use regex::{Regex, RegexSet};

use crate::Test;

pub trait TestFilter<T>: std::fmt::Debug {
    fn is_match(&self, test: &T) -> bool;
}

impl<T, F: TestFilter<T> + ?Sized> TestFilter<T> for Box<F> {
    fn is_match(&self, test: &T) -> bool {
        (**self).is_match(test)
    }
}

impl<T, F: TestFilter<T>> TestFilter<T> for Option<F> {
    fn is_match(&self, test: &T) -> bool {
         self.as_ref().map_or(true, |filter| filter.is_match(test))
    }
}

impl<D> TestFilter<Test<D>> for Regex {
    fn is_match(&self, test: &Test<D>) -> bool {
        self.is_match(&test.name)
    }
}

impl<D> TestFilter<Test<D>> for RegexSet {
    fn is_match(&self, test: &Test<D>) -> bool {
        self.is_match(&test.name)
    }
}
