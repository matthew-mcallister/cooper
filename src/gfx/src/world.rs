use std::sync::Arc;

use crate::*;

#[derive(Debug)]
pub struct RenderWorld {
    crate test_mesh: Arc<RenderMesh>,
    crate debug: Vec<DebugMesh>,
}

impl RenderWorld {
    pub fn new(rloop: &mut RenderLoop) -> Self {
        let binding = BufferBinding::Vertex;
        let pos = rloop.state().buffers.box_slice(binding, &[
            -1.0f32, -1.0, 1.0,
            -1.0, 1.0, 0.5,
            1.0, 1.0, 0.0,
            -1.0, -1.0, 1.0,
            1.0, 1.0, 0.0,
            1.0, -1.0, 0.5,
        ]).into_inner();
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
        Self {
            debug: vec![DebugMesh {
                mesh: Arc::clone(&test_mesh),
                display: DebugDisplay::Depth,
            }],
            test_mesh,
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
