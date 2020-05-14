#![feature(crate_visibility_modifier)]
#![feature(exclusive_range_pattern)]
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

unsafe fn render_world(
    world: &mut RenderWorld,
    mesh: &Mesh,
    material: &Arc<Material>,
) {
    let mut view = SceneView::default();

    let fovy2 = 45.0f32.to_radians();
    let tan_fovy2 = fovy2.tan();
    let tan_fovx2 = 16.0 / 9.0 * tan_fovy2;

    // Calculate camera position and near/far planes, chosen so that
    // the mesh is always fully visible.
    let std::ops::Range { start, end } = mesh.bbox;
    let diam = (0..3).map(|i| { let x = end[i] - start[i]; x * x })
        .sum::<f32>().sqrt();
    let radius = diam / 2.0;
    let mut mid = [0.0; 3];
    (0..3).for_each(|i| mid[i] = (end[i] + start[i]) / 2.0);

    // Increase distance to center a bit in case mesh is spherical
    let dist = 1.1 * radius / fovy2.sin();
    let (z_near, z_far) = (dist - radius, dist + radius);

    let (min_depth, max_depth) = (1.0, 0.0);
    view.perspective = PerspectiveParams {
        z_near, z_far, tan_fovx2, tan_fovy2, min_depth, max_depth,
    };

    view.rot = identity();
    view.pos = [mid[0], mid[1], mid[2] - dist];
    world.set_view(view);

    // Framerate is not bounded yet, so the frequency is kind of
    // arbitrary.
    let t = world.frame_num() as f32 / 60.0;
    let f = 0.2;
    let phi = 2.0 * std::f32::consts::PI * f * t;
    let (c, s) = (phi.cos(), phi.sin());
    let rot = [
        [c, 0.0, s],
        [0.0, 1.0, 0.0],
        [-s, 0.0, c],
    ];

    world.add_instance(MeshInstance {
        /// Assumed to be orthogonal.
        mesh: Arc::clone(&mesh.render_mesh),
        rot,
        pos: Default::default(),
        material: Arc::clone(material),
    });
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

        let rl = RenderLoop::new(app_info, Arc::clone(&window)).unwrap();

        let path = std::env::var("GLTF_PATH")?;
        let bundle = GltfBundle::import(&path)?;
        let mesh = Arc::new(Mesh::from_gltf(&rl, &bundle)?);

        let prog = MaterialProgram::Checker;
        let material = rl.create_material(prog, Default::default());

        let mut rl = Some(Box::new(rl));
        while !window.should_close() {
            let mut world = RenderWorld::new(rl.take().unwrap());
            render_world(&mut world, &mesh, &material);
            rl = Some(world.render());
        }

        Ok(())
    });
}
