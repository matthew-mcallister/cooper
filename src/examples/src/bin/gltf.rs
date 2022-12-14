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

use anyhow::Result as AnyResult;
use asset::{AssetCache, NodeData, Primitive, SceneCollection};
use gfx::{
    MaterialDef, MeshInstance, PerspectiveParams, RenderLoop, RenderWorld,
    SceneView, WorldUniforms,
};
use device::{AppInfo, ShaderStage};
use math::{BBox3, MathItertools, Matrix3, vec3};

use cooper_examples::with_event_loop;

const NAME: &str = "gltf example";

fn render_world(
    world: &mut RenderWorld,
    collection: &SceneCollection,
    materials: &Vec<Vec<MeshMaterials>>,
) {
    let mut view = SceneView::default();

    let fovy2 = 45.0f32.to_radians();
    let tan_fovy2 = fovy2.tan();
    let tan_fovx2 = 16.0 / 9.0 * tan_fovy2;
    let scene = collection.default_scene();

    // TODO: compute transformed AABBs
    let bbox: BBox3 = collection.resources.meshes.iter()
        .flat_map(|mesh| mesh.primitives.iter())
        .map(|prim| prim.bbox)
        .sup()
        .unwrap();

    // Ensure whole scene is visible
    let diam = (bbox.max - bbox.min).length();
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

    let mid = (bbox.min + bbox.max) / 2.0;
    let pos = mid - rot * vec3(0.0, 0.0, dist);

    let v = rot.translate(rot * (-pos));
    view.view = v;

    let mut xforms = collection.world_xforms();
    xforms.iter_mut().for_each(|m| *m = v * *m);

    world.set_uniforms(WorldUniforms { view, xforms });

    let mat_idx = (world.frame_num() / 109) as usize;
    for (node_idx, node) in scene.nodes.iter()
        .map(|&idx| (idx as usize, &collection.nodes[idx as usize]))
    {
        match node.data {
            NodeData::Empty => {},
            NodeData::Mesh(idx) => {
                let idx = idx as usize;
                let mesh = &collection.resources.meshes[idx];
                for (prim, materials) in mesh.primitives.iter()
                    .zip(materials[idx].iter())
                {
                    let material = &materials[mat_idx % materials.len()];
                    let xform_idx = node_idx as u32;
                    render_mesh(world, prim, Arc::clone(material), xform_idx);
                }
            }
        }
    }
}

fn render_mesh(
    world: &mut RenderWorld,
    prim: &Primitive,
    material: Arc<MaterialDef>,
    xform_index: u32,
) {
    world.add_object(MeshInstance {
        mesh: Arc::clone(&prim.mesh),
        material,
        xform_index,
    });
}

type MeshMaterials = [Arc<MaterialDef>; 6];

fn primitive_materials(rl: &mut RenderLoop, prim: &Primitive) -> MeshMaterials
{
    let desc = &prim.material;

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

fn main_with_proxy(proxy: window::EventLoopProxy) -> AnyResult<()> {
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
    let mut rloop = RenderLoop::new(app_info, Arc::clone(&window))?;
    let mut assets = AssetCache::new(&mut rloop);

    let path = std::env::var("GLTF_PATH")?;
    let scene = assets.get_or_load_scene(&mut rloop, &path)?;
    let materials: Vec<Vec<_>> = scene.resources.meshes.iter()
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
