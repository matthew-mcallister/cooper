#![feature(crate_visibility_modifier)]
#![feature(non_exhaustive)]
#![feature(try_blocks)]
use std::ffi::CString;
use std::sync::Arc;

const TITLE_BASE: &'static str = "Cooper Demo";

fn make_title(fps: f32) -> CString {
    let title = format!("{} | {:.2} fps", TITLE_BASE, fps);
    CString::new(title).unwrap()
}

fn main() {
    unsafe { unsafe_main() }
}

unsafe fn unsafe_main() {
    let title = CString::new(TITLE_BASE).unwrap();
    let dims = (1280, 720).into();
    let config = win::Config {
        title: title.as_ptr(),
        dims,
        hints: Default::default(),
    };
    let window = Arc::new(win::Window::new(
        win::System::new().unwrap(),
        config,
    ).unwrap());

    let config = Arc::new(gfx::GraphicsConfig {
        app_name: TITLE_BASE.into(),
        app_version: [0, 1, 0],
        // TODO: Hide behind a CLI arg or env var
        enable_debug_names: true,
    });
    let mut state = gfx::init_video(config, Arc::clone(&window));
    state.load_textures();

    loop {
        state.wait_for_next_frame();

        state.set_sprite_count(2);
        let sprites = state.sprites();
        (*sprites)[0] = gfx::Sprite {
            transform: gfx::SpriteTransform {
                mat: [
                    [0.35355339, -0.35355339],
                    [0.35355339,  0.35355339],
                ],
                offset: [0.0, -0.35355339],
            },
            textures: [0, 0],
        };
        (*sprites)[1] = gfx::Sprite {
            transform: gfx::SpriteTransform {
                mat: [
                    [0.28125, 0.0],
                    [    0.0, 0.5],
                ],
                offset: [0.0, 0.0],
            },
            textures: [0, 0],
        };

        state.render();
        state.present();

        // Update FPS counter
        if state.history_full() {
            let title = make_title(state.compute_fps());
            window.set_title(title.as_ptr());
        }

        window.sys().poll_events();
        if window.should_close() { break; }
    }
}
