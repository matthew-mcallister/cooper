use std::sync::Arc;

use crate::*;

const VERTEX_COUNT: usize = 8;

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
crate struct TrivialInstance {
    crate radius: [f32; 4],
    crate offset: [f32; 4],
    crate rot_cols: [[f32; 4]; 3],
    crate colors: [[f32; 4]; VERTEX_COUNT],
}

#[derive(Debug)]
crate struct TrivialRenderer {
    inst_layout: Arc<SetLayout>,
    pipe_layout: Arc<PipelineLayout>,
    instances: Option<BufferBox<[TrivialInstance]>>,
    descs: DescriptorSet,
}

impl TrivialRenderer {
    crate const fn vertex_count() -> usize {
        VERTEX_COUNT
    }

    crate fn new(globals: &Globals, state: &mut SystemState) -> Self {
        let device = Arc::clone(&state.device);

        let bindings = set_layout_bindings![(0, STORAGE_BUFFER)];
        let inst_layout = unsafe {
            Arc::new(SetLayout::from_bindings(Arc::clone(&device), &bindings))
        };

        let pipe_layout = Arc::new(PipelineLayout::new(device, [
            &globals.scene_global_layout,
            &inst_layout,
        ].iter().cloned().cloned().collect()));

        let descs = state.descriptors.lock().alloc(&inst_layout);

        TrivialRenderer {
            inst_layout,
            pipe_layout,
            instances: None,
            descs,
        }
    }

    crate fn instance_layout(&self) -> &Arc<SetLayout> {
        &self.inst_layout
    }

    crate fn pipeline_layout(&self) -> &Arc<PipelineLayout> {
        &self.pipe_layout
    }
}
