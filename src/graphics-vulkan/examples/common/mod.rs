use std::sync::Arc;
use std::thread;

use cooper_graphics_vulkan::*;

#[macro_export]
macro_rules! include_shader {
    ($name:expr) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            concat!("/generated/shaders/", $name),
        ))
    }
}

pub(crate) unsafe fn init_video(ev: &window::EventLoopProxy, title: &str) ->
    (Arc<Swapchain>, Vec<Vec<Arc<Queue>>>)
{
    let create_info = window::CreateInfo {
        title: title.to_owned(),
        dims: window::Dimensions::new(1280, 720),
        hints: Default::default(),
    };
    let window = Arc::new(ev.create_window(create_info).unwrap());

    let config = GraphicsConfig {
        app_name: title.to_owned(),
        app_version: [0, 1, 0],
        debug: true,
    };
    let vk_platform = window.vk_platform().clone();
    let instance = Arc::new(Instance::new(vk_platform, config).unwrap());
    let surface = instance.create_surface(&window).unwrap();
    let pdev = device_for_surface(&surface).unwrap();
    let (device, queues) = instance.create_device(pdev).unwrap();
    let swapchain = device.create_swapchain(&surface).unwrap();

    (swapchain, queues)
}

pub(crate) unsafe fn with_event_loop<F>(f: F)
where
    F: FnOnce(window::EventLoopProxy) + Send + 'static
{
    let (mut ev_loop, ev_proxy) = window::init().unwrap();

    let thread = thread::spawn(move || f(ev_proxy));

    ev_loop.pump();
    thread.join().unwrap();
}
