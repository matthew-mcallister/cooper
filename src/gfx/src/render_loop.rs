use std::sync::Arc;

use parking_lot::Mutex;

use crate::*;

#[derive(Debug)]
crate struct SystemState {
    crate device: Arc<Device>,
    // TODO: Internal locking
    crate heap: Mutex<DeviceHeap>,
    crate buffers: Mutex<BufferHeap>,
    crate descriptors: Mutex<DescriptorPool>,
    crate gfx_pipes: GraphicsPipelineCache,
    //compute_pipes: ...,
    crate samplers: SamplerCache,
}

impl SystemState {
    crate fn new(device: Arc<Device>) -> Self {
        let heap = Mutex::new(DeviceHeap::new(Arc::clone(&device)));
        let buffers = Mutex::new(BufferHeap::new(Arc::clone(&device)));
        let descriptors =
            Mutex::new(create_global_descriptor_pool(Arc::clone(&device)));
        let gfx_pipes = GraphicsPipelineCache::new(Arc::clone(&device));
        let samplers = SamplerCache::new(Arc::clone(&device));
        SystemState {
            device,
            heap,
            buffers,
            descriptors,
            gfx_pipes,
            samplers,
        }
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }
}
