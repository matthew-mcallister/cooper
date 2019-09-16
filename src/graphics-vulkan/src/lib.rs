#![feature(arbitrary_self_types)]
#![feature(crate_visibility_modifier)]
#![feature(non_exhaustive)]
#![feature(seek_convenience)]
#![feature(try_blocks)]

#[cfg(test)]
macro_rules! test_type {
    () => { crate::testing::VulkanTest }
}

mod debug;
mod descriptor;
mod init;
mod memory;

pub use debug::*;
pub use descriptor::*;
pub use init::*;
pub use memory::*;

unit::collect_tests![
    descriptor,
    init,
    memory,
];

#[cfg(test)]
mod testing;

#[cfg(test)]
fn main() {
    testing::run_tests();
}
