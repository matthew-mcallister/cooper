use super::*;

/// The staging buffer implementation used for transfer operations.
#[derive(Debug)]
crate struct StagingBuffer {
    buffer: DeviceBuffer,
    allocator: LinearAllocator,
}

impl StagingBuffer {
    crate fn new(device: Arc<Device>, capacity: usize) -> Self {
        let buffer = unsafe {
            DeviceBuffer::new(
                device,
                capacity as _,
                vk::BufferUsageFlags::TRANSFER_SRC_BIT,
                MemoryMapping::Mapped,
                Lifetime::Static,
                None,
            )
        };
        let mut allocator = LinearAllocator::default();
        allocator.add_chunk(capacity as _);
        Self {
            buffer,
            allocator,
        }
    }

    crate fn device(&self) -> &Arc<Device> {
        self.buffer.device()
    }

    crate fn inner(&self) -> &DeviceBuffer {
        &self.buffer
    }

    crate fn used(&self) -> usize {
        self.allocator.used() as _
    }

    crate fn capacity(&self) -> usize {
        self.allocator.capacity() as _
    }

    crate fn alloc(&mut self, size: usize) -> Option<BufferRange<'_>> {
        let blk = self.allocator.alloc(size as _, 1)?;
        Some(BufferRange {
            buffer: &self.buffer,
            offset: blk.start,
            size: blk.end - blk.start,
        })
    }

    crate unsafe fn clear(&mut self) {
        self.allocator.clear();
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use super::*;

    fn staging_inner(staging: &mut StagingBuffer) {
        assert_eq!(staging.used(), 0);
        assert_eq!(staging.capacity(), 1024);

        let alloc = staging.alloc(16).unwrap();
        let ptr = alloc.buffer as *const _;
        assert_eq!(alloc.range(), 0..16);
        assert_eq!(staging.used(), 16);
        assert_eq!(staging.capacity(), 1024);

        let alloc = staging.alloc(16).unwrap();
        // Buffer stays the same
        assert_eq!(ptr, alloc.buffer as *const _);
        assert_eq!(alloc.range(), 16..32);
        assert_eq!(staging.used(), 32);
        assert_eq!(staging.capacity(), 1024);

        // Cannot alloc past end of buffer
        assert!(staging.alloc(1000).is_none());
        assert_eq!(staging.used(), 32);
        assert_eq!(staging.capacity(), 1024);

        // Can alloc to end of buffer
        assert_eq!(staging.alloc(992).unwrap().range(), 32..1024);
        assert_eq!(staging.used(), 1024);
        assert_eq!(staging.capacity(), 1024);

        assert!(staging.alloc(8).is_none());
    }

    fn staging(vars: testing::TestVars) {
        let mut staging = StagingBuffer::new(Arc::clone(vars.device()), 1024);

        // Run test, clear, and run it again
        staging_inner(&mut staging);
        unsafe { staging.clear(); }
        staging_inner(&mut staging);
    }

    unit::declare_tests![staging];
}

unit::collect_tests![tests];
