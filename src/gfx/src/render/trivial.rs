use std::sync::Arc;

use crate::*;

const VERTEX_COUNT: usize = 3;

#[derive(Debug)]
crate struct TrivialRenderer {
    set_layouts: [Arc<SetLayout>; 2],
    pipe_layout: Arc<PipelineLayout>,
    descs: [DescriptorSet; 2],
}

impl TrivialRenderer {
    crate const fn vertex_count() -> usize {
        VERTEX_COUNT
    }

    crate fn new(state: &SystemState) -> Self {
        let device = Arc::clone(&state.device);

        let bindings = set_layout_bindings![
            (0, UNIFORM_BUFFER),
            (1, STORAGE_BUFFER),
        ];
        let layout0 = unsafe {
            Arc::new(SetLayout::from_bindings(Arc::clone(&device), &bindings))
        };

        let bindings = set_layout_bindings![
            (0, COMBINED_IMAGE_SAMPLER),
            (1, STORAGE_IMAGE),
        ];
        let layout1 = unsafe {
            Arc::new(SetLayout::from_bindings(Arc::clone(&device), &bindings))
        };

        let pipe_layout = Arc::new(PipelineLayout::new(device, vec![
            Arc::clone(&layout0),
            Arc::clone(&layout1),
        ]));

        let mut descs = state.descriptors.lock();
        let desc0 = descs.alloc(&layout0);
        let desc1 = descs.alloc(&layout1);

        TrivialRenderer {
            set_layouts: [layout0, layout1],
            pipe_layout,
            descs: [desc0, desc1],
        }
    }

    crate fn descriptor_layouts(&self) -> &[Arc<SetLayout>] {
        &self.set_layouts[..]
    }

    crate fn pipeline_layout(&self) -> &Arc<PipelineLayout> {
        &self.pipe_layout
    }

    crate fn descriptors(&self) -> &[DescriptorSet] {
        &self.descs[..]
    }
}
