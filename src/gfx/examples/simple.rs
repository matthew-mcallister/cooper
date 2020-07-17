#![feature(
    crate_visibility_modifier,
    exclusive_range_pattern,
    exact_size_is_empty,
    try_blocks,
    type_ascription,
)]

use std::sync::Arc;

use anyhow as any;
use cooper_gfx::*;
use math::vector::*;
use math::matrix::*;

mod common;

use common::with_event_loop;
use common::gltf::*;

fn main() {
    unsafe { unsafe_main() }
}

const NAME: &'static str = "debug example";

unsafe fn render_world(
    world: &mut RenderWorld,
    mesh: &Mesh,
    materials: &[Arc<MaterialDef>],
) {
    let mut view = SceneView::default();

    let fovy2 = 45.0f32.to_radians();
    let tan_fovy2 = fovy2.tan();
    let tan_fovx2 = 16.0 / 9.0 * tan_fovy2;

    // Use mesh bbox to ensure mesh is always visible.
    let bbox = mesh.bbox;
    let diam = (bbox[1] - bbox[0]).length();
    let radius = diam / 2.0;
    let dist = 1.1 * radius / fovy2.sin(); // Give it a little room
    let (z_near, z_far) = (dist - radius, dist + radius);
    let (min_depth, max_depth) = (1.0, 0.0);

    view.perspective = PerspectiveParams {
        z_near, z_far, tan_fovx2, tan_fovy2, min_depth, max_depth,
    };
    view.rot = Matrix3::identity();
    view.pos = vec3(0.0, 0.0, -dist);
    world.set_view(view);

    // Framerate is not bounded yet, so the frequency is kind of
    // arbitrary.
    let t = world.frame_num() as f32 / 60.0;
    let f = 0.2;
    let phi = 2.0 * std::f32::consts::PI * f * t;
    let (c, s) = (phi.cos(), phi.sin());
    let turntable = Matrix3::from([
        [ c,   0.0, s  ],
        [ 0.0, 1.0, 0.0],
        [-s,   0.0, c  ],
    ]);

    let flip = Matrix3::from([
        [-1.0,  0.0, 0.0],
        [ 0.0, -1.0, 0.0],
        [ 0.0,  0.0, 0.0],
    ]);
    let rot = flip * turntable;

    // p = P R (p_0 - m)
    let mid = (bbox[0] + bbox[1]) / 2.0;
    let pos = -rot * mid;

    let idx = (world.frame_num() / 109) as usize;
    let material = Arc::clone(&materials[idx % materials.len()]);

    world.add_object(MeshInstance {
        /// Assumed to be orthogonal.
        mesh: Arc::clone(&mesh.render_mesh),
        rot,
        pos,
        material,
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

        let mut rl = RenderLoop::new(app_info, Arc::clone(&window)).unwrap();

        let path = std::env::var("GLTF_PATH")?;
        let bundle = GltfBundle::import(&path)?;
        let mesh = Arc::new(Mesh::from_gltf(&mut rl, &bundle)?);

        let materials = [
            rl.define_material(MaterialProgram::Checker, Default::default()),
            rl.define_material(MaterialProgram::FragDepth, Default::default()),
            rl.define_material(MaterialProgram::FragNormal, Default::default()),
            rl.define_material(MaterialProgram::Albedo, mesh.images.clone()),
        ];

        let mut rl = Some(Box::new(rl));
        while !window.should_close() {
            let mut world = RenderWorld::new(rl.take().unwrap());
            render_world(&mut world, &mesh, &materials);
            rl = Some(world.render());
        }

        std::mem::drop(mesh);
        std::mem::drop(materials);

        Ok(())
    });
}
