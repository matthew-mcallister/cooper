use std::sync::Arc;

use cooper_graphics_vulkan::*;

#[macro_use]
mod common;

use common::*;

unsafe fn render_main(ev_proxy: window::EventLoopProxy) {
    let app = AppInfo {
        name: "triangle demo".to_owned(),
        version: [0, 1, 0],
        debug: true,
    };
    let config = Config {
        width: 1280,
        height: 720,
    };
    let mut render_loop = RenderLoop::new(&ev_proxy, app, config).unwrap();
    let window = Arc::clone(render_loop.window());

    while !window.should_close() {
        render_loop.do_frame();
    }
}

fn main() {
    unsafe { with_event_loop(|proxy| render_main(proxy)); }
}
