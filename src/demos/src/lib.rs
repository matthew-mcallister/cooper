#![feature(try_blocks)]

#[macro_export]
macro_rules! c_str {
    ($str:expr) => {
        concat!($str, "\0") as *const str as *const std::os::raw::c_char
    }
}

#[macro_export]
macro_rules! include_shader {
    ($name:expr) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            concat!("/generated/shaders/", $name),
        ))
    }
}

#[macro_export]
macro_rules! assert_success {
    ($expr:expr) => {
        assert_eq!($expr, vk::Result::SUCCESS);
    }
}

mod graphics;
mod init;

pub use graphics::*;
pub use init::*;
