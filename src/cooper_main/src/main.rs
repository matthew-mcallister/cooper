#![feature(crate_visibility_modifier)]
#![feature(non_exhaustive)]
#![feature(optin_builtin_traits)]
#![feature(try_blocks)]
#![feature(uniform_paths)]

#[macro_use]
extern crate derive_more;
extern crate glfw_ffi as glfw;
extern crate lodepng;
extern crate vk_ffi as vk;
extern crate vk_ffi_loader as vkl;

use std::os::raw::c_char;
use std::sync::Arc;

macro_rules! c_str {
    ($str:expr) => {
        concat!($str, "\0") as *const str as *const [c_char] as *const c_char
    }
}

macro_rules! asset {
    ($str:expr) => {
        concat!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/"), $str)
    }
}

fn as_bytes<T: Sized>(src: &T) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            src as *const _ as _,
            std::mem::size_of::<T>(),
        )
    }
}

fn slice_bytes<T: Sized>(src: &[T]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            src.as_ptr() as _,
            src.len() * std::mem::size_of::<T>(),
        )
    }
}

crate mod render;
crate mod window;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    unsafe { unsafe_main() }
}

unsafe fn unsafe_main() -> Result<(), Box<dyn std::error::Error>> {
    let dims = window::Dimensions::new(1600, 900);
    let window = Arc::new(window::Window::new(dims, c_str!("Demo"))?);
    let vk_config = render::VulkanConfig {
        enable_validation: std::env::var("VULKAN_VALIDATE")
            .map(|s| &s == "1")
            .unwrap_or(false),
    };
    let sys = Arc::new(render::VulkanSys::new(vk_config)?);
    let _swapchain = render::VulkanSwapchain::new(Arc::clone(&sys), window)?;
    let _renderer = render::Renderer::new(Arc::clone(&sys))?;

    Ok(())
}
