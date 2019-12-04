#![feature(arbitrary_self_types)]
#![feature(bool_to_option)]
#![feature(crate_visibility_modifier)]
#![feature(seek_convenience)]
#![feature(try_blocks)]

#![allow(dead_code)]

#[cfg(test)]
macro_rules! test_type {
    () => { crate::testing::VulkanTest }
}

mod config;
mod device;

pub use config::*;
pub use device::*;

pub fn clear_color(color: [f32; 4]) -> vk::ClearValue {
    vk::ClearValue {
        color: vk::ClearColorValue {
            float_32: color,
        },
    }
}

unit::collect_tests![
    device,
];

#[cfg(test)]
mod testing;

#[cfg(test)]
fn main() {
    testing::run_tests();
}
