#![feature(crate_visibility_modifier)]
#![feature(non_exhaustive)]
#![feature(optin_builtin_traits)]
#![feature(uniform_paths)]

#[macro_use]
extern crate derive_more;
extern crate glfw_ffi as glfw;
extern crate vk_ffi as vk;
extern crate vk_ffi_loader;

use std::os::raw::c_char;

macro_rules! c_str {
    ($str:expr) => {
        concat!($str, "\0") as *const str as *const [c_char] as *const c_char
    }
}

crate mod window;

use self::window as win;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    unsafe { unsafe_main() }
}

unsafe fn unsafe_main() -> Result<(), Box<dyn std::error::Error>> {
    win::Window::new(
        win::Dimensions::new(640, 480),
        c_str!("Demo"),
    )?;
    Ok(())
}
