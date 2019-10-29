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
mod render_loop;
mod triangle;

pub use cor::*;
pub use render_loop::*;
crate use triangle::*;

pub fn clear_color(color: [f32; 4]) -> vk::ClearValue {
    vk::ClearValue {
        color: vk::ClearColorValue {
            float_32: color,
        },
    }
}

unit::collect_tests![
    cor,
];

#[cfg(test)]
mod testing;

#[cfg(test)]
fn main() {
    testing::run_tests();
}
