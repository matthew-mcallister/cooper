#![feature(arbitrary_self_types)]
#![feature(crate_visibility_modifier)]
#![feature(non_exhaustive)]
#![feature(seek_convenience)]
#![feature(try_blocks)]

#![allow(dead_code)]

#[cfg(test)]
macro_rules! test_type {
    () => { crate::testing::VulkanTest }
}

mod cor;
mod objects_core;

crate use cor::*;
crate use objects_core::*;

unit::collect_tests![
    cor,
    objects_core,
];

#[cfg(test)]
mod testing;

#[cfg(test)]
fn main() {
    testing::run_tests();
}
