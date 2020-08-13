#![feature(
    const_generics,
    crate_visibility_modifier,
    exclusive_range_pattern,
    exact_size_is_empty,
    try_blocks,
    type_ascription,
)]
#![allow(incomplete_features)]

use std::sync::Arc;

use anyhow as any;
use asset::{AssetCache, BBox, Primitive, Scene};
use gfx::{
    MaterialDef, MeshInstance, PerspectiveParams, RenderLoop, RenderWorld,
    SceneView,
};
use device::{AppInfo, ShaderStage};
use math::vector::*;
use math::matrix::*;
use num::One;

use cooper_examples::with_event_loop;

const NAME: &str = "gltf example";

fn render_world(
    world: &mut RenderWorld,
    scene: &Scene,
    materials: &Vec<Vec<MeshMaterials>>,
) {
    let mut view = SceneView::default();

    let fovy2 = 45.0f32.to_radians();
    let tan_fovy2 = fovy2.tan();
    let tan_fovx2 = 16.0 / 9.0 * tan_fovy2;

    let bbox = scene_bbox(scene);

    // Ensure whole scene is visible
    let diam = (bbox[1] - bbox[0]).length();
    let radius = diam / 2.0;
    let dist = 1.1 * radius / fovy2.sin(); // Give it a little room
    let (z_near, z_far) = (dist - radius, dist + radius);
    let (min_depth, max_depth) = (1.0, 0.0);
    view.perspective = PerspectiveParams {
        z_near, z_far, tan_fovx2, tan_fovy2, min_depth, max_depth,
    };

    // Framerate is not bounded yet, so the frequency is kind of
    // arbitrary.
    let t = world.frame_num() as f32 / 60.0;
    let f = 0.2;
    let phi = 2.0 * std::f32::consts::PI * f * t;
    let (c, s) = (phi.cos(), phi.sin());
    let rot = Matrix3::from([
        [-c,    0.0, -s  ],
        [ 0.0, -1.0,  0.0],
        [-s,    0.0,  c  ],
    ]);

    let mid = (bbox[0] + bbox[1]) / 2.0;
    view.rot = rot;
    view.pos = mid - rot * vec3(0.0, 0.0, dist);
    world.set_view(view);

    assert_eq!(scene.meshes.len(), materials.len());
    for (mesh, materials) in scene.meshes.iter().zip(materials.iter()) {
        assert_eq!(mesh.primitives.len(), materials.len());
        for (prim, material) in mesh.primitives.iter().zip(materials.iter()) {
            render_mesh(world, prim, material);
        }
    }
}

fn scene_bbox(scene: &Scene) -> BBox {
    let mut min = vec([f32::INFINITY; 3]);
    let mut max = vec([-f32::INFINITY; 3]);
    for prim in scene.primitives() {
        min = vec_min(min, prim.bbox[0]);
        max = vec_max(max, prim.bbox[1]);
    }
    [min, max]
}

fn vec_min<const N: usize>(u: Vector<f32, N>, v: Vector<f32, N>) ->
    Vector<f32, N>
{
    let mut min: Vector<f32, N> = Default::default();
    for i in 0..N {
        min[i] = if u[i] < v[i] { u[i] } else { v[i] }
    }
    min
}

fn vec_max<const N: usize>(u: Vector<f32, N>, v: Vector<f32, N>) ->
    Vector<f32, N>
{
    let mut min: Vector<f32, N> = Default::default();
    for i in 0..N {
        min[i] = if u[i] > v[i] { u[i] } else { v[i] }
    }
    min
}

fn render_mesh(
    world: &mut RenderWorld,
    prim: &Primitive,
    materials: &MeshMaterials,
) {
    let idx = (world.frame_num() / 109) as usize;
    let material = Arc::clone(&materials[idx % materials.len()]);
    world.add_object(MeshInstance {
        /// Assumed to be orthogonal.
        mesh: Arc::clone(&prim.mesh),
        pos: Default::default(),
        rot: Matrix3::one(),
        material,
    });
}

type MeshMaterials = [Arc<MaterialDef>; 6];

fn primitive_materials(rl: &mut RenderLoop, prim: &Primitive) -> MeshMaterials
{
    let desc = prim.material.desc().clone();

    macro_rules! define_material {
        ($rl:expr, $base:expr, $frag_stage:ident) => {
            {
                let mut desc = $base.clone();
                desc.stages[ShaderStage::Fragment] =
                    Arc::clone(&$rl.specs().$frag_stage);
                $rl.define_material(&desc)
            }
        }
    }

    [
        define_material!(rl, desc, checker_frag),
        define_material!(rl, desc, geom_depth_frag),
        define_material!(rl, desc, geom_normal_frag),
        define_material!(rl, desc, albedo_frag),
        define_material!(rl, desc, tex_normal_frag),
        define_material!(rl, desc, metal_rough_frag),
    ]
}

fn main() {
    unsafe { with_event_loop(main_with_proxy); }
}

fn main_with_proxy(proxy: window::EventLoopProxy) -> any::Result<()> {
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
    let mut rloop = RenderLoop::new(app_info, Arc::clone(&window)).unwrap();
    let mut assets = AssetCache::new();

    let path = std::env::var("GLTF_PATH")?;
    let scene = assets.get_or_load_scene(&mut rloop, &path)?;
    let materials: Vec<Vec<_>> = scene.meshes.iter()
        .map(|mesh| mesh.primitives.iter()
            .map(|prim| primitive_materials(&mut rloop, prim))
            .collect())
        .collect();

    let mut rloop = Some(Box::new(rloop));
    while !window.should_close() {
        let mut world = RenderWorld::new(rloop.take().unwrap());
        render_world(&mut world, &scene, &materials);
        rloop = Some(world.render());
    }

    std::mem::drop(scene);
    std::mem::drop(materials);

    Ok(())
}
