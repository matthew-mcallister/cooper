use std::sync::Arc;

use crate::*;

#[derive(Debug)]
crate struct SceneViewState {
    state: Arc<Box<SystemState>>,
    crate uniforms: SceneViewUniforms,
    crate uniform_buffer: BufferBox<SceneViewUniforms>,
}

// TODO: Override Default
#[derive(Clone, Copy, Debug, Default)]
pub struct SceneView {
    pub perspective: PerspectiveParams,
    pub rot: [[f32; 3]; 3],
    pub pos: [f32; 3],
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
    crate proj: [[f32; 4]; 4],
    //crate proj_inv: [[f32; 4]; 4],
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
    crate view: [[f32; 4]; 4],
    crate view_inv: [[f32; 4]; 4],
    // TODO:
    //crate view_proj: [[f32; 4]; 4],
    //crate view_proj_inv: [[f32; 4]; 4],
}

impl SceneViewState {
    crate fn new(state: Arc<Box<SystemState>>, world: &RenderWorld) -> Self {
        let view = world.view;

        let view_inv = affine_xform(view.rot, view.pos);
        let view_mat = rigid_xform_inv(view.rot, view.pos);

        let uniforms = SceneViewUniforms {
            perspective: view.perspective.into(),
            view: view_mat,
            view_inv,
        };
        let uniform_buffer = state.buffers.boxed(
            BufferBinding::Uniform,
            Lifetime::Frame,
            uniforms,
        );

        Self {
            state,
            uniforms,
            uniform_buffer,
        }
    }

    crate fn state(&self) -> &SystemState {
        &self.state
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

/// Calculates a column-major perspective matrix.
crate fn perspective(params: PerspectiveParams) -> [[f32; 4]; 4] {
    let (z_n, z_f) = (params.z_near, params.z_far);
    let (d_n, d_f) = (params.min_depth, params.max_depth);
    let (s_x, s_y) = (params.tan_fovx2, params.tan_fovy2);
    let c = z_f * (d_f - d_n) / (z_f - z_n);
    [
        [1.0 / s_x, 0.0,       0.0,      0.0],
        [0.0,       1.0 / s_y, 0.0,      0.0],
        [0.0,       0.0,       c + d_n,  1.0],
        [0.0,       0.0,       -z_n * c, 0.0],
    ]
}
