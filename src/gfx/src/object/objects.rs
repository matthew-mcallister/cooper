use std::sync::Arc;

use derive_more::From;

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
    /// Selects which transform matrix to apply.
    pub xform_index: u32,
}
