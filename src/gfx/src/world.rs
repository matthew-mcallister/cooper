use std::sync::Arc;

use crate::*;

#[derive(Debug)]
pub struct RenderWorld {
    crate test_mesh: Arc<RenderMesh>,
    crate debug: Vec<DebugMesh>,
    crate perspective: PerspectiveUniforms,
}

impl RenderWorld {
    pub fn new(rloop: &mut RenderLoop) -> Self {
        let binding = BufferBinding::Vertex;
        let lifetime = Lifetime::Frame;

        let [p0, p1, p2, p3] = [
            [0.2, -5.0, 10.0],
            [0.2,  5.0, 10.0],
            [0.2, -5.0,  0.0],
            [0.2,  5.0,  0.0],
        ];
        let data = [p0, p1, p3, p0, p3, p2];
        let data: &[f32] = flatten_arrays(&data);
        let pos = rloop.state().buffers.box_slice(binding, lifetime, data)
            .into_inner();
        let test_mesh = Arc::new(RenderMesh {
            tri_count: 2,
            index: None,
            bindings: enum_map! {
                VertexAttrName::Position => Some(AttrBuffer {
                    alloc: pos,
                    format: Format::RGB32F,
                }),
                _ => None,
            },
        });

        let znear = 0.1;
        let zfar = 10.0;
        let tanfovy2 = 45.0f32.to_radians().tan();
        let tanfovx2 = 16.0 / 9.0 * tanfovy2;
        let proj = perspective(tanfovx2, tanfovy2, znear, zfar, 1.0, 0.0);
        let perspective = PerspectiveUniforms {
            tanfovx2, tanfovy2, znear, zfar, proj,
        };

        Self {
            debug: vec![DebugMesh {
                mesh: Arc::clone(&test_mesh),
                display: DebugDisplay::Depth,
                mv: identity(),
            }],
            test_mesh,
            perspective,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn smoke_test(vars: crate::testing::TestVars) {
        let window = Arc::clone(&vars.swapchain.surface.window);
        let app_info = (*vars.device().instance.app_info).clone();
        let mut rl = RenderLoop::new(app_info, window).unwrap();
        let world = RenderWorld::new(&mut rl);
        rl.render(world);
    }

    unit::declare_tests![smoke_test];
}

unit::collect_tests![tests];
