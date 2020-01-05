use std::ptr;
use std::sync::Arc;

use enum_map::EnumMap;
use prelude::*;

use crate::*;
use super::*;

#[derive(Debug)]
crate struct HeapPool {
    device: Arc<Device>,
    type_index: u32,
    tiling: Tiling,
    allocator: FreeListAllocator,
    chunks: Vec<Arc<DeviceMemory>>,
}

// TODO maybe: Multithread this so all memory can be RAII
#[derive(Debug)]
crate struct DeviceHeap {
    device: Arc<Device>,
    // One pool per memory type per tiling
    pools: Vec<EnumMap<Tiling, HeapPool>>,
}

impl Drop for HeapPool {
    fn drop(&mut self) {
        for chunk in self.chunks.iter() {
            assert_eq!(Arc::strong_count(chunk), 1,
                "allocator destroyed while chunk in use: {:?}", chunk);
        }
    }
}

impl HeapPool {
    fn new(
        device: Arc<Device>,
        type_index: u32,
        tiling: Tiling,
    ) -> Self {
        HeapPool {
            device,
            type_index,
            tiling,
            allocator: Default::default(),
            chunks: Vec::new(),
        }
    }

    fn used(&self) -> vk::DeviceSize {
        self.allocator.used()
    }

    fn reserved(&self) -> vk::DeviceSize {
        self.allocator.capacity()
    }

    fn memory_type(&self) -> &vk::MemoryType {
        &self.device.mem_props
            .memory_types[self.type_index as usize]
    }

    fn heap_index(&self) -> u32 {
        self.memory_type().heap_index
    }

    fn has_flags(&self, flags: vk::MemoryPropertyFlags) -> bool {
        self.memory_type()
            .property_flags
            .contains(flags)
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

    unsafe fn add_chunk(&mut self, min_size: vk::DeviceSize) {
        let chunk = self.chunks.len() as u32;
        let size = align(self.chunk_size(), min_size);
        let inner = alloc_device_memory(&self.device, size, self.type_index);
        let mut mem = DeviceMemory {
            device: Arc::clone(&self.device),
            inner,
            size,
            type_index: self.type_index,
            ptr: 0 as _,
            tiling: self.tiling,
            chunk,
        };
        mem.init();
        self.chunks.push(Arc::new(mem));
        self.allocator.add_chunk(size);
    }

    unsafe fn alloc(&mut self, size: vk::DeviceSize, alignment: vk::DeviceSize)
        -> DeviceAlloc
    {
        let alignment = std::cmp::max(self.min_alignment(), alignment);
        let size = align(alignment, size);
        let block = self.allocator.alloc(size, alignment)
            .or_else(|| {
                self.add_chunk(size);
                self.allocator.alloc(size, alignment)
            })
            .unwrap();
        let chunk = block.chunk;
        let memory = Arc::clone(&self.chunks[chunk as usize]);
        DeviceAlloc {
            memory,
            offset: block.offset(),
            size: block.size(),
        }
    }

    fn free(&mut self, alloc: DeviceAlloc) {
        // Make sure the allocation came from this pool
        assert!(Arc::ptr_eq(
            &alloc.memory,
            &self.chunks[alloc.memory.chunk as usize],
        ));
        self.allocator.free(to_block(&alloc));
    }

    fn clear(&mut self) {
        for chunk in self.chunks.iter() {
            assert_eq!(Arc::strong_count(chunk), 1,
                "chunk cleared while in use: {:?}", chunk);
        }
        self.allocator.clear()
    }
}

impl DeviceHeap {
    crate fn new(device: Arc<Device>) -> Self {
        let pools: Vec<EnumMap<_, _>> = iter_memory_types(&device)
            .enumerate()
            .map(|(idx, _)| (|tiling| HeapPool::new(
                Arc::clone(&device),
                idx as _,
                tiling,
            )).into())
            .collect();
        DeviceHeap {
            device,
            pools,
        }
    }

    fn chunk_size() -> vk::DeviceSize {
        0x100_0000
    }

    fn pool_mut(&mut self, type_idx: u32, tiling: Tiling) -> &mut HeapPool {
        &mut self.pools[type_idx as usize][tiling]
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    fn dt(&self) -> &vkl::DeviceTable {
        &*self.device.table
    }

    crate fn heaps(&self) -> Vec<HeapInfo> {
        let heap_count = self.device.mem_props.memory_heap_count as usize;
        let mut heaps = vec![HeapInfo::default(); heap_count];
        for pool in self.pools.iter().flat_map(|x| x.values()) {
            let heap = &mut heaps[pool.heap_index() as usize];
            heap.reserved += pool.reserved();
            heap.used += pool.used();
        }
        heaps
    }

    crate unsafe fn alloc(
        &mut self,
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
        self.pool_mut(type_idx, tiling).alloc(reqs.size, reqs.alignment)
    }

    /// Binds a buffer to newly allocated memory.
    crate unsafe fn alloc_buffer_memory(
        &mut self,
        buffer: vk::Buffer,
        mapping: MemoryMapping,
    ) -> DeviceAlloc {
        let mut reqs = Default::default();
        self.dt().get_buffer_memory_requirements(buffer, &mut reqs);
        let alloc = self.alloc(reqs, Tiling::Linear, mapping);

        let memory = alloc.memory().inner();
        let offset = alloc.offset();
        self.dt().bind_buffer_memory(buffer, memory, offset).check().unwrap();

        alloc
    }

    /// Binds an image to newly allocated memory.
    // TODO: Image tiling is assumed nonlinear
    crate unsafe fn alloc_image_memory(
        &mut self,
        image: vk::Image,
        mapping: MemoryMapping,
    ) -> DeviceAlloc {
        let mut reqs = Default::default();
        self.dt().get_image_memory_requirements(image, &mut reqs);
        let alloc = self.alloc(reqs, Tiling::Nonlinear, mapping);

        let memory = alloc.memory().inner();
        let offset = alloc.offset();
        self.dt().bind_image_memory(image, memory, offset).check().unwrap();

        alloc
    }

    // TODO: Allocations might be freed via RAII
    crate fn free(&mut self, alloc: DeviceAlloc) {
        let type_index = alloc.memory().type_index();
        let tiling = alloc.memory().tiling();
        self.pool_mut(type_index, tiling).free(alloc);
    }
}

#[cfg(test)]
mod tests {
    use vk::traits::*;
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        use Tiling::*;
        use MemoryMapping::*;

        let device = Arc::clone(&vars.swapchain.device);
        let mut heap = DeviceHeap::new(Arc::clone(&device));

        let reqs = vk::MemoryRequirements {
            size: 4096,
            alignment: 256,
            memory_type_bits: !0,
        };
        let _alloc0 = heap.alloc(reqs, Linear, Mapped);
        let _alloc1 = heap.alloc(reqs, Nonlinear, Unmapped);
        assert_ne!(_alloc0.as_raw(), 0 as _);
        heap.free(_alloc0);
    }

    unit::declare_tests![smoke_test];
}

unit::collect_tests![tests];
