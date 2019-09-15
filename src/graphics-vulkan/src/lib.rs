#![feature(arbitrary_self_types)]
#![feature(crate_visibility_modifier)]
#![feature(seek_convenience)]
#![feature(try_blocks)]

#[cfg(test)]
macro_rules! test_type {
    () => { crate::testing::VulkanTest }
}

mod debug;
mod init;

pub use debug::*;
pub use init::*;

unit::collect_tests![
    init,
];

#[cfg(test)]
mod testing;

#[cfg(test)]
fn main() {
    testing::run_tests();
}
