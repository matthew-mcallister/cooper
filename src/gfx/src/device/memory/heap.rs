use std::sync::Arc;

use enum_map::EnumMap;
use parking_lot::Mutex;
use prelude::*;

use super::*;

/// A suballocation of a VkMemory object.
#[derive(Clone, Debug)]
crate struct DeviceAlloc {
    memory: Arc<DeviceMemory>,
    offset: vk::DeviceSize,
    size: vk::DeviceSize,
    pool: Option<Arc<HeapPool>>,
}

impl Drop for DeviceAlloc {
    fn drop(&mut self) {
        if let Some(ref pool) = self.pool {
            unsafe { pool.free(self); }
        }
    }
}

impl MemoryRegion for DeviceAlloc {
    fn memory(&self) -> &Arc<DeviceMemory> {
        &self.memory
    }

    fn offset(&self) -> vk::DeviceSize {
        self.offset
    }

    fn size(&self) -> vk::DeviceSize {
        self.size
    }
}

impl DeviceAlloc {
    fn whole_range(memory: Arc<DeviceMemory>) -> Self {
        Self {
            memory,
            offset: 0,
            size: 0,
            pool: None,
        }
    }
}

#[derive(Debug)]
pub(super) struct HeapPool {
    device: Arc<Device>,
    type_index: u32,
    tiling: Tiling,
    inner: Mutex<HeapPoolInner>,
}

#[derive(Debug)]
struct HeapPoolInner {
    allocator: FreeListAllocator,
    chunks: Vec<Arc<DeviceMemory>>,
}

#[derive(Debug)]
crate struct DeviceHeap {
    device: Arc<Device>,
    // One pool per memory type per tiling
    pools: Vec<EnumMap<Tiling, Arc<HeapPool>>>,
}

impl Drop for HeapPoolInner {
    fn drop(&mut self) {
        for chunk in self.chunks.iter() {
            assert_eq!(Arc::strong_count(chunk), 1,
                "allocator destroyed while chunk in use: {:?}", chunk);
        }
    }
}

impl HeapPool {
    fn new(device: Arc<Device>, type_index: u32, tiling: Tiling) -> Self {
        HeapPool {
            device,
            type_index,
            tiling,
            inner: Mutex::new(HeapPoolInner {
                allocator: Default::default(),
                chunks: Vec::new(),
            }),
        }
    }

    /// Returns a tuple (used, reserved) of memory usage numbers.
    fn usage(&self) -> (vk::DeviceSize, vk::DeviceSize) {
        let inner = self.inner.lock();
        (inner.allocator.used(), inner.allocator.capacity())
    }

    fn memory_type(&self) -> &vk::MemoryType {
        &self.device.mem_props
            .memory_types[self.type_index as usize]
    }

    fn heap_index(&self) -> u32 {
        self.memory_type().heap_index
    }

    fn has_flags(&self, flags: vk::MemoryPropertyFlags) -> bool {
        self.memory_type().property_flags.contains(flags)
    }

    fn host_visible(&self) -> bool {
        self.has_flags(vk::MemoryPropertyFlags::HOST_VISIBLE_BIT)
    }

    fn mapped(&self) -> bool {
        self.host_visible()
    }

    fn chunk_size(&self) -> vk::DeviceSize {
        0x100_0000
    }

    fn min_alignment(&self) -> vk::DeviceSize {
        32
    }

    unsafe fn add_chunk(
        &self,
        inner: &mut HeapPoolInner,
        min_size: vk::DeviceSize,
    ) {
        let chunk = inner.chunks.len() as u32;
        // TODO: Possibly size should be a power-of-two of chunk size
        let size = align(self.chunk_size(), min_size);
        let mem = alloc_device_memory(&self.device, &vk::MemoryAllocateInfo {
            allocation_size: size,
            memory_type_index: self.type_index,
            ..Default::default()
        });
        let mut mem = DeviceMemory {
            device: Arc::clone(&self.device),
            inner: mem,
            size,
            type_index: self.type_index,
            ptr: 0 as _,
            tiling: self.tiling,
            dedicated_content: None,
            chunk,
        };
        mem.init();
        inner.chunks.push(Arc::new(mem));
        inner.allocator.add_chunk(size);
    }

    unsafe fn alloc(
        self: &Arc<Self>,
        size: vk::DeviceSize,
        alignment: vk::DeviceSize,
    ) -> DeviceAlloc {
        let alignment = std::cmp::max(self.min_alignment(), alignment);
        let size = align(alignment, size);
        let mut inner = self.inner.lock();
        let block = inner.allocator.alloc(size, alignment)
            .or_else(|| {
                self.add_chunk(&mut *inner, size);
                inner.allocator.alloc(size, alignment)
            })
            .unwrap();
        let chunk = block.chunk;
        let memory = Arc::clone(&inner.chunks[chunk as usize]);
        std::mem::drop(inner);
        DeviceAlloc {
            memory,
            offset: block.offset(),
            size: block.size(),
            pool: Some(Arc::clone(self)),
        }
    }

    unsafe fn free(&self, alloc: &DeviceAlloc) {
        let mut inner = self.inner.lock();
        // Make sure the allocation came from this pool
        assert!(Arc::ptr_eq(
            &alloc.memory,
            &inner.chunks[alloc.memory.chunk as usize],
        ));
        inner.allocator.free(to_block(alloc));
    }

    fn clear(&mut self) {
        let mut inner = self.inner.lock();
        for chunk in inner.chunks.iter() {
            assert_eq!(Arc::strong_count(chunk), 1,
                "chunk cleared while in use: {:?}", chunk);
        }
        inner.allocator.clear()
    }
}

impl DeviceHeap {
    crate fn new(device: Arc<Device>) -> Self {
        let pools: Vec<EnumMap<_, _>> = iter_memory_types(&device)
            .enumerate()
            .map(|(idx, _)| (|tiling| Arc::new(HeapPool::new(
                Arc::clone(&device),
                idx as _,
                tiling,
            ))).into())
            .collect();
        Self {
            device,
            pools,
        }
    }

    fn chunk_size() -> vk::DeviceSize {
        0x100_0000
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    fn dt(&self) -> &vkl::DeviceTable {
        &*self.device.table
    }

    fn pool(&self, type_idx: u32, tiling: Tiling) -> &Arc<HeapPool> {
        &self.pools[type_idx as usize][tiling]
    }

    // N.B. This races with other threads.
    crate fn heaps(&self) -> Vec<HeapInfo> {
        let heap_count = self.device.mem_props.memory_heap_count as usize;
        let mut heaps = vec![HeapInfo::default(); heap_count];
        for pool in self.pools.iter().flat_map(|x| x.values()) {
            let heap = &mut heaps[pool.heap_index() as usize];
            let (used, reserved) = pool.usage();
            heap.used += used;
            heap.reserved += reserved;
        }
        heaps
    }

    /// Suballocates device memory.
    crate unsafe fn alloc(
        &self,
        reqs: vk::MemoryRequirements,
        tiling: Tiling,
        mapping: MemoryMapping,
    ) -> DeviceAlloc {
        // TODO: fall back to incoherent memory on failure
        let type_idx = find_memory_type(
            &*self.device,
            mapping.memory_property_flags(),
            reqs.memory_type_bits,
        ).unwrap();
        self.pool(type_idx, tiling).alloc(reqs.size, reqs.alignment)
    }

    /// Binds a buffer to newly allocated memory.
    crate unsafe fn alloc_buffer_memory(
        &self,
        buffer: vk::Buffer,
        mapping: MemoryMapping,
    ) -> DeviceAlloc {
        let (reqs, dedicated_reqs) =
            get_buffer_memory_reqs(&self.device, buffer);

        let alloc = if dedicated_reqs.prefers_dedicated_allocation == vk::TRUE
        {
            DeviceAlloc::whole_range(Arc::new(alloc_resource_memory(
                Arc::clone(&self.device),
                mapping,
                &reqs,
                Some(DedicatedAllocContent::Buffer(buffer)),
                Tiling::Linear,
            )))
        } else { self.alloc(reqs, Tiling::Linear, mapping) };

        let memory = alloc.memory().inner();
        let offset = alloc.offset();
        self.dt().bind_buffer_memory(buffer, memory, offset).check().unwrap();

        alloc
    }

    /// Binds an image to newly allocated memory.
    crate unsafe fn alloc_image_memory(
        &self,
        image: vk::Image,
        mapping: MemoryMapping,
    ) -> DeviceAlloc {
        let (reqs, dedicated_reqs) =
            get_image_memory_reqs(&self.device, image);

        let alloc = if dedicated_reqs.prefers_dedicated_allocation == vk::TRUE
        {
            DeviceAlloc::whole_range(Arc::new(alloc_resource_memory(
                Arc::clone(&self.device),
                mapping,
                &reqs,
                Some(DedicatedAllocContent::Image(image)),
                Tiling::Nonlinear,
            )))
        } else { self.alloc(reqs, Tiling::Nonlinear, mapping) };

        let memory = alloc.memory().inner();
        let offset = alloc.offset();
        self.dt().bind_image_memory(image, memory, offset).check().unwrap();

        alloc
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use vk::traits::*;
    use crate::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        use Tiling::*;
        use MemoryMapping::*;

        let device = Arc::clone(&vars.swapchain.device);
        let heap = DeviceHeap::new(Arc::clone(&device));

        let reqs = vk::MemoryRequirements {
            size: 4096,
            alignment: 256,
            memory_type_bits: !0,
        };
        let _alloc0 = heap.alloc(reqs, Linear, Mapped);
        let _alloc1 = heap.alloc(reqs, Nonlinear, Unmapped);
        assert_ne!(_alloc0.as_raw(), 0 as _);
    }

    unit::declare_tests![smoke_test];
}

unit::collect_tests![tests];
