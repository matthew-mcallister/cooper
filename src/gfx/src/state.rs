use std::sync::Arc;

use device::{
    BufferHeap, DescriptorHeap, Device, ImageHeap, PipelineCache, SamplerCache,
    SetLayoutCache,
};
use log::trace;

#[derive(Debug)]
crate struct SystemState {
    crate device: Arc<Device>,
    crate buffers: Arc<BufferHeap>,
    crate image_heap: Arc<ImageHeap>,
    crate descriptors: Arc<DescriptorHeap>,
    crate set_layouts: SetLayoutCache,
    crate pipelines: PipelineCache,
    crate samplers: SamplerCache,
    //shader_specs: ..., (maybe)
}

impl SystemState {
    crate fn new(device: Arc<Device>) -> Self {
        let dev = || Arc::clone(&device);
        let buffers = BufferHeap::new(dev());
        let image_heap = Arc::new(ImageHeap::new(dev()));
        let descriptors = Arc::new(DescriptorHeap::new(&device));
        let set_layouts = SetLayoutCache::new(dev());
        let pipelines = PipelineCache::new(&device);
        let samplers = SamplerCache::new(dev());
        SystemState {
            device,
            buffers,
            image_heap,
            descriptors,
            set_layouts,
            pipelines,
            samplers,
        }
    }

    #[allow(dead_code)]
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
