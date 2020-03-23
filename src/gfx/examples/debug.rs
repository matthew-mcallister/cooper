use std::sync::Arc;

use cooper_gfx::*;

mod common;

use common::*;

fn main() {
    unsafe { unsafe_main(); }
}

const NAME: &'static str = "debug example";

fn identity() -> [[f32; 3]; 3] {
    [
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ]
}

unsafe fn render_world(world: &mut RenderWorld) {
    let [p0, p1, p2, p3] = [
        [0.2, -5.0, 10.0f32],
        [0.2,  5.0, 10.0],
        [0.2, -5.0,  0.0],
        [0.2,  5.0,  0.0],
    ];
    let positions = [p0, p1, p3, p0, p3, p2];
    let mut builder = RenderMeshBuilder::new(world);
    builder.tri_count(2)
        .lifetime(Lifetime::Frame)
        .vertex(VertexAttrName::Position, Format::RGB32F, &positions);
    let mesh = Arc::new(builder.build());
    world.add_debug(DebugMesh {
        mesh,
        display: DebugDisplay::Depth,
        rot: identity(),
        pos: Default::default(),
    });

    let mut view = SceneView::default();

    let (z_near, z_far) = (0.1, 10.0);
    let tan_fovy2 = 45.0f32.to_radians().tan();
    let tan_fovx2 = 16.0 / 9.0 * tan_fovy2;
    let (min_depth, max_depth) = (1.0, 0.0);
    view.perspective = PerspectiveParams {
        z_near, z_far, tan_fovx2, tan_fovy2, min_depth, max_depth,
    };

    view.rot = identity();
    view.pos = Default::default();
    world.set_view(view);
}

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
            let mut world = RenderWorld::new(&mut rl);
            render_world(&mut world);
            rl.render(world);
        }
    });
}
