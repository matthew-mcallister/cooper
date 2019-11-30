use std::sync::Arc;

use crate::*;

#[derive(Debug)]
crate struct Image {
    // May be null if image is not in use
    crate inner: vk::Image,
    // May be null if image is not in use
    crate view: vk::ImageView,
    crate extent: vk::Extent3D,
    crate format: vk::Format,
    crate dst_layout: vk::ImageLayout,
    crate dst_access_mask: vk::AccessFlags,
    // TODO: Calculate from extent and format
    crate size: usize,
    crate batch_serial: Option<XferBatchSerial>,
    crate bound_alloc: Option<DeviceAlloc>,
}

crate unsafe fn create_image_mem(
    device: Arc<Device>,
    base_size: vk::DeviceSize,
) -> Box<MemoryPool> {
    let mem_flags = vk::MemoryPropertyFlags::DEVICE_LOCAL_BIT;
    let type_index = find_memory_type(&device, mem_flags).unwrap();
    let create_info = MemoryPoolCreateInfo {
        type_index,
        base_size,
        ..Default::default()
    };
    Box::new(MemoryPool::new(device, create_info))
}
