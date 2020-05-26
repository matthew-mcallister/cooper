#![feature(crate_visibility_modifier)]
#![feature(exclusive_range_pattern)]
#![feature(exact_size_is_empty)]
#![feature(try_blocks)]

use std::sync::Arc;

use anyhow as any;
use cooper_gfx::*;
use math::vector::*;
use math::matrix::*;

mod common;

use common::*;

fn main() {
    unsafe { unsafe_main() }
}

const NAME: &'static str = "debug example";

unsafe fn render_world(
    world: &mut RenderWorld,
    mesh: &Mesh,
    materials: &[Arc<Material>],
) {
    let mut view = SceneView::default();

    let fovy2 = 45.0f32.to_radians();
    let tan_fovy2 = fovy2.tan();
    let tan_fovx2 = 16.0 / 9.0 * tan_fovy2;

    // Calculate camera position and near/far planes, chosen so that
    // the mesh is always fully visible.
    let (start, end) = (vec(mesh.bbox.start), vec(mesh.bbox.end));
    let diam = (end - start).length();
    let radius = diam / 2.0;
    let mid = (end + start) / 2.0;

    // Increase distance to center a bit in case mesh is spherical
    let dist = 1.1 * radius / fovy2.sin();
    let (z_near, z_far) = (dist - radius, dist + radius);

    let (min_depth, max_depth) = (1.0, 0.0);
    view.perspective = PerspectiveParams {
        z_near, z_far, tan_fovx2, tan_fovy2, min_depth, max_depth,
    };

    view.rot = Matrix3::identity();
    view.pos = mid - vec3(0.0, 0.0, dist);
    world.set_view(view);

    // Framerate is not bounded yet, so the frequency is kind of
    // arbitrary.
    let t = world.frame_num() as f32 / 60.0;
    let f = 0.2;
    let phi = 2.0 * std::f32::consts::PI * f * t;
    let (c, s) = (phi.cos(), phi.sin());
    let rot = Matrix3::from([
        [c, 0.0, s],
        [0.0, 1.0, 0.0],
        [-s, 0.0, c],
    ]);

    let idx = (world.frame_num() / 109) as usize;
    let material = &materials[idx % materials.len()];
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

        let materials = [
            rl.create_material(MaterialProgram::Checker, Default::default()),
            rl.create_material(MaterialProgram::FragDepth, Default::default()),
            rl.create_material(MaterialProgram::FragNormal, Default::default()),
        ];

        let mut rl = Some(Box::new(rl));
        while !window.should_close() {
            let mut world = RenderWorld::new(rl.take().unwrap());
            render_world(&mut world, &mesh, &materials);
            rl = Some(world.render());
        }

        // Manually dropping things sucks; for that reason it seems
        // better to dynamically initialize them inside the main loop.
        std::mem::drop(mesh);
        std::mem::drop(materials);

        Ok(())
    });
}
