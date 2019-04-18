#![feature(crate_visibility_modifier)]
#![feature(non_exhaustive)]
#![feature(try_blocks)]
use std::error::Error;
use std::sync::Arc;

macro_rules! c_str {
    ($str:expr) => {
        concat!($str, "\0") as *const str as *const std::os::raw::c_char
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    unsafe { unsafe_main() }
}

unsafe fn unsafe_main() -> Result<(), Box<dyn Error>> {
    let wconfig = window::Config {
        title: c_str!("Demo"),
        dims: window::Dimensions::new(1600, 900),
    };
    let wsys = window::System::new()?;
    let window = Arc::new(window::Window::new(wsys.clone(), wconfig)?);

    let rconfig = render::Config {
        enable_validation: std::env::var("VULKAN_VALIDATE")
            .map(|s| &s == "1")
            .unwrap_or(false),
    };
    let instance = Arc::new(render::Instance::new(rconfig)?);
    let surface =
        Arc::new(render::Surface::new(instance, Arc::clone(&window))?);
    let device = Arc::new(render::Device::new(&surface)?);
    let swapchain = render::Swapchain::new(surface, device)?;

    render::do_test(&swapchain)?;

    while !window.should_close() {
        //do_frame();
        wsys.poll_events();
    }

    Ok(())
}
