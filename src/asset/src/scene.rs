use std::sync::Arc;

use gfx::{MaterialDesc, RenderMesh};
use math::BBox;

/// This type holds all objects necessary to instantiate a scene at
/// runtime. It is shared between multiple `SceneDefs`.
#[derive(Debug)]
pub struct SceneResources {
    pub meshes: Vec<Mesh>,
}

#[derive(Debug)]
pub struct Mesh {
    pub primitives: Vec<Primitive>,
}

#[derive(Debug)]
pub struct Primitive {
    pub bbox: BBox<f32, 3>,
    pub mesh: Arc<RenderMesh>,
    pub material: MaterialDesc,
}
