#![feature(crate_visibility_modifier)]
#![feature(exact_size_is_empty)]
#![feature(try_blocks)]

use std::sync::Arc;

use anyhow as any;
use cooper_gfx::*;

mod common;

use common::*;

fn main() {
    unsafe { unsafe_main() }
}

const NAME: &'static str = "debug example";

fn identity() -> [[f32; 3]; 3] {
    [
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ]
}

unsafe fn render_world(world: &mut RenderWorld, mesh: Arc<RenderMesh>) {
    world.add_debug(DebugMesh {
        mesh,
        display: DebugDisplay::Checker,
        // TODO: rot is a misnomer
        // TODO: Vary with time/input
        rot: [
            [-10.0, 0.0, 0.0],
            [0.0, -10.0, 0.0],
            [0.0, 0.0, -10.0],
        ],
        pos: [0.0, 0.25, -0.4],
        colors: [[1.0, 0.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0]],
    });

    let mut view = SceneView::default();

    let (z_near, z_far) = (0.1, 3.0);
    let tan_fovy2 = 45.0f32.to_radians().tan();
    let tan_fovx2 = 16.0 / 9.0 * tan_fovy2;
    let (min_depth, max_depth) = (1.0, 0.0);
    view.perspective = PerspectiveParams {
        z_near, z_far, tan_fovx2, tan_fovy2, min_depth, max_depth,
    };

    view.rot = identity();
    view.pos = [0.0, 0.0, -1.0];
    world.set_view(view);
}

unsafe fn unsafe_main() {
    with_event_loop::<any::Error, _>(|proxy| {
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

        let path = std::env::var("GLTF_PATH")?;
        let bundle = GltfBundle::import(&path)?;
        let mesh = Arc::new(Mesh::from_gltf(&rl, &bundle)?.render_mesh);

        while !window.should_close() {
            let mut world = RenderWorld::new(&mut rl);
            render_world(&mut world, Arc::clone(&mesh));
            rl.render(world);
        }

        Ok(())
    });
}
