use std::sync::Arc;

use cooper_gfx::*;

mod common;

use common::*;

fn main() {
    unsafe { unsafe_main(); }
}

const NAME: &'static str = "trivial example";

unsafe fn unsafe_main() {
    with_event_loop(|proxy| {
        let info = window::CreateInfo {
            title: NAME.to_owned(),
            dims: (1280, 768).into(),
            hints: Default::default(),
        };
        let window = Arc::new(proxy.create_window(info).unwrap());

        let app_info = AppInfo {
            name: NAME.to_owned(),
            version: [0, 1, 0],
            debug: true,
            ..Default::default()
        };

        let mut rl = RenderLoop::new(app_info, Arc::clone(&window)).unwrap();

        while !window.should_close() {
            rl.do_frame();
        }
    });
}
