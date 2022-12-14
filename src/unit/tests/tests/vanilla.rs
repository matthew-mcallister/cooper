// N.B. Some tests are borrowed from the hashbrown crate.
use std::collections::HashSet;

use cooper_unit::*;

macro_rules! test_type {
    () => { PlainTest }
}

pub(crate) fn test_disjoint() {
    let mut xs = HashSet::new();
    let mut ys = HashSet::new();
    assert!(xs.is_disjoint(&ys));
    assert!(ys.is_disjoint(&xs));
    assert!(xs.insert(5));
    assert!(ys.insert(11));
    assert!(xs.is_disjoint(&ys));
    assert!(ys.is_disjoint(&xs));
    assert!(xs.insert(7));
    assert!(xs.insert(19));
    assert!(xs.insert(4));
    assert!(ys.insert(2));
    assert!(ys.insert(-11));
    assert!(xs.is_disjoint(&ys));
    assert!(ys.is_disjoint(&xs));
    assert!(ys.insert(7));
    assert!(!xs.is_disjoint(&ys));
    assert!(!ys.is_disjoint(&xs));
}

pub(crate) fn test_subset_and_superset() {
    let mut a = HashSet::new();
    assert!(a.insert(0));
    assert!(a.insert(5));
    assert!(a.insert(11));
    assert!(a.insert(7));

    let mut b = HashSet::new();
    assert!(b.insert(0));
    assert!(b.insert(7));
    assert!(b.insert(19));
    assert!(b.insert(250));
    assert!(b.insert(11));
    assert!(b.insert(200));

    assert!(!a.is_subset(&b));
    assert!(!a.is_superset(&b));
    assert!(!b.is_subset(&a));
    assert!(!b.is_superset(&a));

    assert!(b.insert(5));

    assert!(a.is_subset(&b));
    assert!(!a.is_superset(&b));
    assert!(!b.is_subset(&a));
    assert!(b.is_superset(&a));
}

pub(crate) fn test_iterate() {
    let mut a = HashSet::new();
    for i in 0..32 {
        assert!(a.insert(i));
    }
    let mut observed: u32 = 0;
    for k in &a {
        observed |= 1 << *k;
    }
    assert_eq!(observed, 0xFFFF_FFFF);
}

pub(crate) fn test_ignore() {
    unimplemented!();
}

pub(crate) fn test_xfail() {
    unimplemented!();
}

pub(crate) fn test_ignore_xfail() {
    unimplemented!();
}

pub(crate) fn test_should_err() {
    let s = HashSet::<u32>::new();
    assert!(s.contains(&15));
}

pub(crate) fn test_failure() {
    let mut s = HashSet::new();
    s.insert(15u32);
    assert!(!s.contains(&15));
}

pub(crate) fn test_xpass() {
    let mut s = HashSet::new();
    s.insert(15u32);
    assert!(s.contains(&15));
}

declare_tests![
    test_disjoint,
    test_subset_and_superset,
    test_iterate,
    (#[ignore] test_ignore),
    (#[xfail] test_xfail),
    (#[ignore] #[xfail] test_ignore_xfail),
    (#[should_err] test_should_err),
    test_failure,
    (#[xfail] test_xpass),
];
