use math::matrix::*;

#[derive(Debug)]
crate struct SceneViewState {
    crate uniforms: SceneViewUniforms,
}

// TODO: Override Default
#[derive(Clone, Copy, Debug, Default)]
pub struct SceneView {
    pub perspective: PerspectiveParams,
    pub view: Matrix4,
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

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
crate struct SceneViewUniforms {
    crate perspective: PerspectiveUniforms,
    /// Transforms from world space to view space.
    crate view: Matrix4,
    // Transforms from view space to world space.
    //crate view_inv: Matrix4,
}

impl SceneViewUniforms {
    crate fn new(view: &SceneView) -> SceneViewUniforms {
        SceneViewUniforms {
            perspective: view.perspective.into(),
            view: view.view,
        }
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
