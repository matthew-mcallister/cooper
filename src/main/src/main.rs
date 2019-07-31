#![feature(crate_visibility_modifier)]
#![feature(non_exhaustive)]
#![feature(try_blocks)]
use std::ffi::CString;
use std::sync::Arc;

use common::*;

mod asset;

pub use asset::*;

const TITLE_BASE: &'static str = "Cooper Demo";

fn make_title(fps: f32) -> CString {
    let title = format!("{} | {:.2} fps", TITLE_BASE, fps);
    CString::new(title).unwrap()
}

fn fixed_mul8(a: u8, b: u8) -> u8 {
    let a = a as u16;
    let b = b as u16;
    ((a * b + 128) / 255) as u8
}

fn premultiply_alpha(img: &mut lodepng::Bitmap<lodepng::RGBA>) {
    for pixel in img.buffer.iter_mut() {
        pixel.r = fixed_mul8(pixel.r, pixel.a);
        pixel.g = fixed_mul8(pixel.g, pixel.a);
        pixel.b = fixed_mul8(pixel.b, pixel.a);
    }
}

fn load_textures(assets: &AssetManager, state: &mut gfx::RenderState) {
    let mut source = assets.open("enemy/beetle/sprites/walk000.png")
        .unwrap().unwrap();
    let mut data = Vec::new();
    source.read_to_end(&mut data).unwrap();
    let mut img = lodepng::decode32(&data).unwrap();
    premultiply_alpha(&mut img);
    let extent = vk::Extent3D::new(img.width as _, img.height as _, 1);
    let format = vk::Format::R8G8B8A8_SRGB;
    let data = slice_to_bytes(&img.buffer[..]);
    unsafe { state.textures.load_image(extent, format, data); }
}

fn main() {
    unsafe { unsafe_main() }
}

unsafe fn unsafe_main() {
    let assets_dir = std::env::var("ASSETS").unwrap();
    let bundle = DirectoryAssetBundle::new(assets_dir).unwrap();
    let mut asset_man = AssetManager::new();
    asset_man.add_bundle(Box::new(bundle));

    let title = CString::new(TITLE_BASE).unwrap();
    let dims = (1280, 720).into();
    let config = win::Config {
        title: title.as_ptr(),
        dims,
        hints: Default::default(),
    };
    // TODO: Hide window until ready to render
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

    load_textures(&mut asset_man, &mut state);

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
