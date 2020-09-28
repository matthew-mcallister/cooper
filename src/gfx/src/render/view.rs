use device::*;
use math::matrix::*;
use math::vector::*;

use crate::*;

#[derive(Debug)]
crate struct SceneViewState {
    crate uniforms: SceneViewUniforms,
    crate force_cull_mode: Option<CullMode>,
}

// TODO: Override Default
// TODO: Give clearer names to rot and pos
#[derive(Clone, Copy, Debug, Default)]
pub struct SceneView {
    pub perspective: PerspectiveParams,
    /// Rotation of view camera.
    pub rot: Matrix3,
    /// Position of view camera.
    pub pos: Vector3,
    /// For debugging
    pub force_cull_mode: Option<CullMode>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PerspectiveParams {
    pub tan_fovx2: f32,
    pub tan_fovy2: f32,
    pub z_near: f32,
    pub z_far: f32,
    pub min_depth: f32,
    pub max_depth: f32,
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C, align(16))]
crate struct PerspectiveUniforms {
    crate proj: Matrix4,
    //crate proj_inv: Matrix4,
    crate tan_fovx2: f32,
    crate tan_fovy2: f32,
    crate z_near: f32,
    crate z_far: f32,
    crate min_depth: f32,
    crate max_depth: f32,
}

// TODO: Give clearer names to view and view_pos
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
crate struct SceneViewUniforms {
    crate perspective: PerspectiveUniforms,
    /// Transforms from world space to view space.
    crate view: Matrix4,
    /// Transforms from view space to world space.
    crate view_inv: Matrix4,
    // TODO:
    //crate view_proj: [[f32; 4]; 4],
    //crate view_proj_inv: [[f32; 4]; 4],
}

impl SceneViewState {
    crate fn new(view: &SceneView) -> Self {
        Self {
            uniforms: view_uniforms(view),
            force_cull_mode: view.force_cull_mode,
        }
    }

    crate fn write_descriptor(
        &self,
        state: &SystemState,
        descriptors: &mut SceneDescriptors,
    ) {
        let uniform_buffer = state.buffers.boxed(
            BufferBinding::Uniform, Lifetime::Frame, self.uniforms);
        descriptors.write_view_uniforms(BufferBox::range(&uniform_buffer));
    }
}

fn view_uniforms(view: &SceneView) -> SceneViewUniforms {
    let view_inv = view.rot.translate(view.pos);
    let rot_inv = view.rot.transpose();
    let view_mat = rot_inv.translate(rot_inv * (-view.pos));
    SceneViewUniforms {
        perspective: view.perspective.into(),
        view: view_mat,
        view_inv,
    }
}

impl From<PerspectiveParams> for PerspectiveUniforms {
    fn from(params: PerspectiveParams) -> Self {
        let (tan_fovx2, tan_fovy2) = (params.tan_fovx2, params.tan_fovy2);
        let (z_near, z_far) = (params.z_near, params.z_far);
        let (min_depth, max_depth) = (params.min_depth, params.max_depth);
        PerspectiveUniforms {
            proj: perspective(params),
            tan_fovx2, tan_fovy2,
            z_near, z_far,
            min_depth, max_depth,
        }
    }
}

/// Calculates a perspective matrix.
crate fn perspective(params: PerspectiveParams) -> Matrix4 {
    let (z_n, z_f) = (params.z_near, params.z_far);
    let (d_n, d_f) = (params.min_depth, params.max_depth);
    let (s_x, s_y) = (params.tan_fovx2, params.tan_fovy2);
    let c = z_f * (d_f - d_n) / (z_f - z_n);
    [
        [1.0 / s_x, 0.0,       0.0,      0.0],
        [0.0,       1.0 / s_y, 0.0,      0.0],
        [0.0,       0.0,       c + d_n,  1.0],
        [0.0,       0.0,       -z_n * c, 0.0],
    ].into()
}
