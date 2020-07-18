use std::sync::Arc;

use log::trace;

use crate::{
    BufferHeap, DescriptorHeap, Device, ImageHeap, PipelineCache, SamplerCache,
};

#[derive(Debug)]
crate struct SystemState {
    crate device: Arc<Device>,
    crate heap: ImageHeap,
    crate buffers: Arc<BufferHeap>,
    crate descriptors: Arc<DescriptorHeap>,
    crate pipelines: PipelineCache,
    crate samplers: SamplerCache,
    //shader_specs: ..., (maybe)
}

impl SystemState {
    crate fn new(device: Arc<Device>) -> Self {
        let dev = || Arc::clone(&device);
        let heap = ImageHeap::new(dev());
        let buffers = BufferHeap::new(dev());
        let descriptors = Arc::new(DescriptorHeap::new(&device));
        let pipelines = PipelineCache::new(&device);
        let samplers = SamplerCache::new(dev());
        SystemState {
            device,
            heap,
            buffers,
            descriptors,
            pipelines,
            samplers,
        }
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    crate fn frame_over(&mut self) {
        trace!("SystemState::frame_over(...)");
        unsafe {
            self.buffers.clear_frame();
            self.descriptors.clear_frame();
        }
        self.pipelines.commit();
        self.samplers.commit();
    }
}
