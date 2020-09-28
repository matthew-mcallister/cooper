use std::sync::Arc;

use derive_more::From;
use math::{Matrix3, Vector3, Matrix4};

use crate::RenderMesh;
use crate::material::MaterialDef;

#[derive(Clone, Debug, From)]
pub enum RenderObject {
    MeshInstance(MeshInstance),
}

// TODO maybe: Validate mesh is compatible with material vertex shader
#[derive(Clone, Debug)]
pub struct MeshInstance {
    pub mesh: Arc<RenderMesh>,
    pub material: Arc<MaterialDef>,
    /// Assumed to be orthogonal.
    pub rot: Matrix3,
    pub pos: Vector3,
    //TODO:
    //pub scale: f32,
}

impl MeshInstance {
    pub fn xform(&self) -> Matrix4 {
        self.rot.translate(self.pos)
    }
}
