#![feature(crate_visibility_modifier)]
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

mod descriptor;
mod frame;
mod init;
mod master;
mod memory;
mod object;
mod render_path;
mod sprite;
mod stats;

use descriptor::*;
use frame::*;
use init::*;
use master::*;
use memory::*;
use object::*;
use render_path::*;
use sprite::*;
use stats::*;

#[inline(always)]
#[allow(dead_code)]
crate fn align(alignment: usize, offset: usize) -> usize {
    ((offset + alignment - 1) / alignment) * alignment
}

#[inline(always)]
crate fn align_64(alignment: u64, offset: u64) -> u64 {
    ((offset + alignment - 1) / alignment) * alignment
}

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

        state.set_sprite_count(2);
        let sprites = state.sprites();
        (*sprites)[0] = Sprite {
            transform: SpriteTransform {
                mat: [
                    [0.35355339, -0.35355339],
                    [0.35355339,  0.35355339],
                ],
                offset: [0.0, -0.35355339],
            },
            textures: [0, 0],
        };
        (*sprites)[1] = Sprite {
            transform: SpriteTransform {
                mat: [
                    [0.28125, 0.0],
                    [    0.0, 0.5],
                ],
                offset: [0.0, 0.0],
            },
            textures: [0, 0],
        };

        state.render(queue, present_sem);
        state.present(queue);

        // Update FPS counter
        if state.history_full() {
            let title = make_title(state.compute_fps());
            window.set_title(title.as_ptr());
        }

        window.sys().poll_events();
        if window.should_close() { break; }
    }
}
