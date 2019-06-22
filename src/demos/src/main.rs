#![feature(try_blocks)]

macro_rules! c_str {
    ($str:expr) => {
        concat!($str, "\0") as *const str as *const std::os::raw::c_char
    }
}

macro_rules! include_shader {
    ($name:expr) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            concat!("/generated/shaders/", $name),
        ))
    }
}

use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::Arc;

mod frame;
mod init;
mod master;
mod object;
mod render_path;
mod stats;

use frame::*;
use init::*;
use master::*;
use object::*;
use render_path::*;
use stats::*;

const TITLE_BASE: &'static str = "Triangle demo\0";

fn make_title(fps: f32) -> CString {
    let title_base = &TITLE_BASE[..TITLE_BASE.len() - 1];
    let title = format!("{} | {:.2} fps", title_base, fps);
    unsafe { CString::from_vec_unchecked(title.into()) }
}

fn app_title() -> *const c_char {
    TITLE_BASE.as_ptr() as _
}

fn main() {
    unsafe { unsafe_main() }
}

unsafe fn unsafe_main() {
    let swapchain = init_video();

    let window = Arc::clone(&swapchain.surface.window);
    let device = Arc::clone(&swapchain.device);
    let queue = device.get_queue(0, 0);

    let mut state = RenderState::new(swapchain);

    let mut objs = Box::new(ObjectTracker::new(Arc::clone(&device)));

    // ???: Can this be doubly used when graphics/present are split?
    let present_sem = objs.create_semaphore();

    loop {
        state.wait_for_next_frame(present_sem);

        // Update FPS counter
        if state.history_full() {
            let title = make_title(state.compute_fps());
            window.set_title(title.as_ptr());
        }

        state.render(queue, present_sem);
        state.present(queue);

        window.sys().poll_events();
        if window.should_close() { break; }
    }
}
