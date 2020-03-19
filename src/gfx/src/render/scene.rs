use std::sync::Arc;

use crate::*;

#[derive(Clone, Copy, Debug, Default)]
#[repr(C, align(16))]
crate struct PerspectiveUniforms {
    crate tanfovx2: f32,
    crate tanfovy2: f32,
    crate znear: f32,
    crate zfar: f32,
    crate proj: [[f32; 4]; 4],
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
crate struct SceneViewUniforms {
    crate perspective: PerspectiveUniforms,
}

#[derive(Debug)]
crate struct SceneView {
    crate state: Arc<SystemState>,
    crate uniforms: BufferBox<SceneViewUniforms>,
    crate cull_mode: vk::CullModeFlags,
}

impl SceneView {
    crate fn new(state: Arc<SystemState>, world: &RenderWorld) -> Self {
        let uniforms = state.buffers.boxed(
            BufferBinding::Uniform,
            Lifetime::Frame,
            SceneViewUniforms { perspective: world.perspective },
        );
        Self {
            state,
            uniforms,
            cull_mode: vk::CullModeFlags::BACK_BIT,
        }
    }

    crate fn state(&self) -> &SystemState {
        &self.state
    }

    crate fn view_uniforms(&self) -> BufferRange<'_> {
        self.uniforms.range()
    }
}

/// Calculates a column-major perspective matrix.
crate fn perspective(
    tan_fovx2: f32,
    tan_fovy2: f32,
    z_near: f32,
    z_far: f32,
    min_depth: f32,
    max_depth: f32,
) -> [[f32; 4]; 4] {
    let (z_n, z_f) = (z_near, z_far);
    let (d_n, d_f) = (min_depth, max_depth);
    let (s_x, s_y) = (tan_fovx2, tan_fovy2);
    let c = z_f * (d_f - d_n) / (z_f - z_n);
    [
        [1.0 / s_x, 0.0,       0.0,      0.0],
        [0.0,       1.0 / s_y, 0.0,      0.0],
        [0.0,       0.0,       c + d_n,  1.0],
        [0.0,       0.0,       -z_n * c, 0.0],
    ]
}

crate fn identity() -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}
