use std::sync::Arc;

use derive_more::From;
use math::Matrix4;

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
    /// Must be decomposible into scale + rotation + translation.
    pub xform: Matrix4,
}

impl MeshInstance {
    pub fn xform(&self) -> Matrix4 {
        self.xform
    }
}
