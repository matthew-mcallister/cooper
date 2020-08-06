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
use cooper_gfx::*;
use log::debug;
use math::vector::*;
use math::matrix::*;

mod common;

use common::with_event_loop;
use common::gltf::*;

const NAME: &'static str = "gltf example";

unsafe fn render_world(
    world: &mut RenderWorld,
    meshes: &[Mesh],
    materials: &[MeshMaterials],
) {
    assert!(!meshes.is_empty());
    assert_eq!(meshes.len(), materials.len());

    let mut view = SceneView::default();

    let fovy2 = 45.0f32.to_radians();
    let tan_fovy2 = fovy2.tan();
    let tan_fovx2 = 16.0 / 9.0 * tan_fovy2;

    let bbox = scene_bbox(meshes);

    // Ensure whole scene is visible
    let diam = (bbox[1] - bbox[0]).length();
    let radius = diam / 2.0;
    let dist = 1.1 * radius / fovy2.sin(); // Give it a little room
    let (z_near, z_far) = (dist - radius, dist + radius);
    let (min_depth, max_depth) = (1.0, 0.0);
    view.perspective = PerspectiveParams {
        z_near, z_far, tan_fovx2, tan_fovy2, min_depth, max_depth,
    };

    let mid = (bbox[0] + bbox[1]) / 2.0;
    let rot = Matrix3::from([
        [-1.0,  0.0, 0.0],
        [ 0.0, -1.0, 0.0],
        [ 0.0,  0.0, 1.0],
    ]);
    view.rot = rot;
    view.pos = rot * (mid - vec3(0.0, 0.0, dist));
    world.set_view(view);

    for (mesh, material) in meshes.iter().zip(materials.iter()) {
        render_mesh(world, mesh, material);
    }
}

fn scene_bbox<'a>(meshes: impl IntoIterator<Item = &'a Mesh>) -> BBox {
    let mut min = vec([f32::INFINITY; 3]);
    let mut max = vec([-f32::INFINITY; 3]);
    for bbox in meshes.into_iter().map(|mesh| mesh.bbox) {
        min = vec_min(min, bbox[0]);
        max = vec_max(max, bbox[1]);
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

unsafe fn render_mesh(
    world: &mut RenderWorld,
    mesh: &Mesh,
    materials: &MeshMaterials,
) {
    // Framerate is not bounded yet, so the frequency is kind of
    // arbitrary.
    let t = world.frame_num() as f32 / 60.0;
    let f = 0.2;
    let phi = 2.0 * std::f32::consts::PI * f * t;
    let (c, s) = (phi.cos(), phi.sin());
    let rot = Matrix3::from([
        [ c,   0.0, s  ],
        [ 0.0, 1.0, 0.0],
        [-s,   0.0, c  ],
    ]);
    let pos = Default::default();

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

type MeshMaterials = [Arc<MaterialDef>; 6];

fn mesh_materials(rl: &mut RenderLoop, mesh: &Mesh) -> MeshMaterials {
    use MaterialProgram::*;

    let vertex_layout = mesh.render_mesh.static_layout();

    let checker_mat = rl.define_material(&MaterialDesc {
        vertex_layout: vertex_layout.clone(),
        program: Checker,
        ..Default::default()
    });
    let geom_depth_mat = rl.define_material(&MaterialDesc {
        vertex_layout: vertex_layout.clone(),
        program: GeomDepth,
        ..Default::default()
    });
    let geom_normal_mat = rl.define_material(&MaterialDesc {
        vertex_layout: vertex_layout.clone(),
        program: GeomNormal,
        ..Default::default()
    });

    let albedo_mat = rl.define_material(&MaterialDesc {
        vertex_layout: vertex_layout.clone(),
        program: Albedo,
        image_bindings: mesh.images.clone(),
    });
    let normal_mat = rl.define_material(&MaterialDesc {
        vertex_layout: vertex_layout.clone(),
        program: NormalMap,
        image_bindings: mesh.images.clone(),
    });
    let met_rough_mat = rl.define_material(&MaterialDesc {
        vertex_layout: vertex_layout.clone(),
        program: MetallicRoughness,
        image_bindings: mesh.images.clone(),
    });

    [
        checker_mat,
        geom_depth_mat,
        geom_normal_mat,
        albedo_mat,
        normal_mat,
        met_rough_mat,
    ]
}

fn main() {
    unsafe { with_event_loop(main_with_proxy); }
}

fn main_with_proxy(proxy: window::EventLoopProxy) -> any::Result<()> {
    unsafe { unsafe_main_with_proxy(proxy) }
}

unsafe fn unsafe_main_with_proxy(proxy: window::EventLoopProxy) ->
    any::Result<()>
{
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

    let path = std::env::var("GLTF_PATH")?;
    let bundle = GltfBundle::import(&path)?;
    let meshes = bundle.load_meshes(&mut rloop)?;
    debug!("meshes: {:?}", meshes);
    let materials: Vec<_> = meshes.iter()
        .map(|mesh| mesh_materials(&mut rloop, mesh))
        .collect();

    let mut rloop = Some(Box::new(rloop));
    while !window.should_close() {
        let mut world = RenderWorld::new(rloop.take().unwrap());
        render_world(&mut world, &meshes, &materials);
        rloop = Some(world.render());
    }

    std::mem::drop(meshes);
    std::mem::drop(materials);

    Ok(())
}
