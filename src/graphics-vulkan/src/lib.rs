#![feature(crate_visibility_modifier)]
#![feature(seek_convenience)]
#![feature(try_blocks)]

macro_rules! include_shader {
    ($name:expr) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            concat!("/generated/shaders/", $name),
        ))
    }
}

mod debug;
mod descriptor;
mod frame;
mod init;
mod master;
mod memory;
mod object;
mod render_path;
mod sprite;
mod stats;
mod texture;

pub use debug::*;
pub use descriptor::*;
pub use frame::*;
pub use init::*;
pub use master::*;
pub use memory::*;
pub use object::*;
pub use render_path::*;
pub use sprite::*;
pub use stats::*;
pub use texture::*;
