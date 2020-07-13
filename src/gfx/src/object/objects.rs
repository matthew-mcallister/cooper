use std::sync::Arc;

use derive_more::From;
use math::{Matrix3, Vector3, Matrix4};

use crate::RenderMesh;
use crate::material::MaterialDef;

#[derive(Clone, Debug, From)]
pub enum RenderObject {
    MeshInstance(MeshInstance),
}

#[derive(Clone, Debug)]
pub struct MeshInstance {
    pub mesh: Arc<RenderMesh>,
    pub material: Arc<MaterialDef>,
    /// Assumed to be orthogonal.
    pub rot: Matrix3<f32>,
    pub pos: Vector3<f32>,
    //TODO:
    //pub scale: f32,
}

impl MeshInstance {
    pub fn xform(&self) -> Matrix4<f32> {
        self.rot.translate(self.pos)
    }
}
