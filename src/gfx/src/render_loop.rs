use std::sync::Arc;

use parking_lot::Mutex;

use crate::*;

#[derive(Debug)]
crate struct SystemState {
    crate device: Arc<Device>,
    crate globals: Globals,
    // TODO: Internal locking
    crate heap: Mutex<DeviceHeap>,
    crate buffers: Mutex<BufferHeap>,
    //images: ...,
    //attachments: ...,
    crate descriptors: Mutex<DescriptorPool>,
    crate gfx_pipes: GraphicsPipelineCache,
    //compute_pipes: ...,
}

impl SystemState {
    crate fn new(device: Arc<Device>) -> Self {
        let globals = Globals::new(Arc::clone(&device));
        let heap = Mutex::new(DeviceHeap::new(Arc::clone(&device)));
        let buffers = Mutex::new(BufferHeap::new(Arc::clone(&device)));
        let descriptors =
            Mutex::new(create_global_descriptor_pool(Arc::clone(&device)));
        let gfx_pipes = GraphicsPipelineCache::new(Arc::clone(&device));
        SystemState {
            device,
            globals,
            heap,
            buffers,
            descriptors,
            gfx_pipes,
        }
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }
}
