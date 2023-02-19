use std::sync::Arc;

use device::{
    BufferHeap, DescriptorHeap, Device, ImageHeap, PipelineCache, SamplerCache, SetLayoutCache,
};
use log::trace;

#[derive(Debug)]
pub(crate) struct SystemState {
    pub(crate) device: Arc<Device>,
    pub(crate) buffers: Arc<BufferHeap>,
    pub(crate) image_heap: Arc<ImageHeap>,
    pub(crate) descriptors: Arc<DescriptorHeap>,
    pub(crate) set_layouts: SetLayoutCache,
    pub(crate) pipelines: PipelineCache,
    pub(crate) samplers: SamplerCache,
    //shader_specs: ..., (maybe)
}

impl SystemState {
    pub(crate) fn new(device: Arc<Device>) -> Self {
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
    pub(crate) fn device(&self) -> &Arc<Device> {
        &self.device
    }

    pub(crate) fn frame_over(&mut self) {
        trace!("SystemState::frame_over(...)");
        unsafe {
            self.buffers.clear_frame();
            self.descriptors.clear_frame();
        }
        self.pipelines.commit();
        self.samplers.commit();
        self.set_layouts.commit();
    }
}
