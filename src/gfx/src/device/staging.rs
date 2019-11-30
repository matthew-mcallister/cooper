use std::sync::Arc;

use crate::*;

#[derive(Debug)]
crate struct StagingBuffer {
    capacity: usize,
    memory: MemoryPool,
}

impl StagingBuffer {
    crate unsafe fn new(device: Arc<Device>, capacity: usize) -> Self {
        let mem_flags = vk::MemoryPropertyFlags::HOST_VISIBLE_BIT;
        let type_index = find_memory_type(&device, mem_flags).unwrap();
        let create_info = MemoryPoolCreateInfo {
            type_index,
            base_size: capacity as _,
            host_mapped: true,
            buffer_map: Some(BufferMapOptions {
                usage: vk::BufferUsageFlags::TRANSFER_SRC_BIT,
            }),
            ..Default::default()
        };
        let memory = MemoryPool::new(device, create_info);

        StagingBuffer {
            memory,
            capacity,
        }
    }

    crate unsafe fn allocate(&mut self, size: usize) -> Option<DeviceAlloc> {
        if self.used() + size <= self.capacity {
            Some(self.memory.allocate(size as _, 1))
        } else { None }
    }

    crate fn clear(&mut self) {
        self.memory.clear();
    }

    crate fn capacity(&self) -> usize {
        self.capacity
    }

    crate fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity;
    }

    crate fn used(&self) -> usize {
        self.memory.used() as _
    }
}

#[cfg(test)]
mod tests {
    use vk::traits::*;
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let device = Arc::clone(vars.swapchain.device());

        let mut staging = StagingBuffer::new(device, 0x1_0000);
        assert_eq!(staging.capacity(), 0x1_0000);

        let alloc0 = staging.allocate(0x8000).unwrap();
        let alloc1 = staging.allocate(0x8000).unwrap();

        let info0 = alloc0.info();
        let info1 = alloc1.info();

        assert!(!info0.memory.is_null());
        assert_eq!(info0.memory, info1.memory);

        assert!(!info0.ptr.is_null());
        assert!(!info0.buffer.is_null());

        assert_eq!(staging.used(), 0x1_0000);

        assert!(staging.allocate(1).is_none());

        staging.clear();
        assert_eq!(staging.used(), 0);
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
