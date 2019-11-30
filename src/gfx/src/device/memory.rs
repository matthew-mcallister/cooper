use std::ops::Range;
use std::ffi::c_void;
use std::ptr;
use std::sync::Arc;

use enum_map::{Enum, EnumMap};
use prelude::*;

use crate::*;

#[inline]
fn compatible_type(type_bits: u32, type_index: u32) -> bool {
    type_bits & (1 << type_index) > 0
}

#[inline]
fn iter_memory_types(device: &Device) -> impl Iterator<Item = &vk::MemoryType>
{
    let props = &device.mem_props;
    props.memory_types.iter().take(props.memory_type_count as _)
}

/// Finds a desirable memory type that meets requirements. This
/// function follows the guidelines in the Vulkan spec stating that
/// implementations are to sort memory types in order of "performance",
/// so the first memory type with the required properties is probably
/// the best for general use.
crate fn find_memory_type(
    device: &Device,
    flags: vk::MemoryPropertyFlags,
    type_mask: u32,
) -> Option<u32> {
    iter_memory_types(device)
        .enumerate()
        .filter(|&(idx, _)| compatible_type(type_mask, idx as u32))
        .position(|(_, ty)| ty.property_flags.contains(flags))
        .map(|x| x as u32)
}

#[inline(always)]
crate fn visible_coherent_memory() -> vk::MemoryPropertyFlags {
    vk::MemoryPropertyFlags::HOST_VISIBLE_BIT |
        vk::MemoryPropertyFlags::HOST_COHERENT_BIT
}

#[derive(Clone, Copy, Debug, Enum, Eq, PartialEq)]
crate enum Tiling {
    /// Denotes a linear image or a buffer.
    Linear,
    /// Denotes a nonlinear (a.k.a. optimal) image.
    Nonlinear,
}

impl From<Tiling> for vk::ImageTiling {
    fn from(tiling: Tiling) -> Self {
        match tiling {
            Tiling::Linear => vk::ImageTiling::LINEAR,
            Tiling::Nonlinear => vk::ImageTiling::OPTIMAL,
        }
    }
}

impl From<vk::ImageTiling> for Tiling {
    fn from(tiling: vk::ImageTiling) -> Self {
        match tiling {
            vk::ImageTiling::LINEAR => Tiling::Linear,
            _ => Tiling::Nonlinear,
        }
    }
}

#[derive(Clone, Copy, Debug)]
crate struct DeviceMemory {
    inner: vk::DeviceMemory,
    size: vk::DeviceSize,
    type_index: u32,
    ptr: *mut c_void,
    tiling: Tiling,
    chunk: u32,
}

impl DeviceMemory {
    crate fn inner(&self) -> vk::DeviceMemory {
        self.inner
    }

    crate fn size(&self) -> vk::DeviceSize {
        self.size
    }

    crate fn type_index(&self) -> u32 {
        self.type_index
    }

    /// Memory-mapped pointer when host-visible.
    crate fn ptr(&self) -> *mut c_void {
        self.ptr
    }

    crate fn tiling(&self) -> Tiling {
        self.tiling
    }

    unsafe fn map(&mut self, device: &Device) {
        assert!(self.ptr.is_null());
        let dt = &*device.table;
        let flags = Default::default();
        dt.map_memory(self.inner, 0, self.size, flags, &mut self.ptr)
            .check().expect("failed to map device memory");
    }
}

crate unsafe fn alloc_device_memory(
    device: &Device,
    size: vk::DeviceSize,
    type_index: u32,
) -> vk::DeviceMemory {
    let dt = &*device.table;
    let alloc_info = vk::MemoryAllocateInfo {
        allocation_size: size,
        memory_type_index: type_index,
        ..Default::default()
    };
    let mut memory = vk::null();
    dt.allocate_memory(&alloc_info, ptr::null(), &mut memory)
        .check().expect("failed to allocate device memory");
    memory
}

#[derive(Clone, Debug)]
crate struct DeviceRange {
    memory: Arc<DeviceMemory>,
    offset: vk::DeviceSize,
    size: vk::DeviceSize,
}

impl DeviceRange {
    crate fn memory(&self) -> &Arc<DeviceMemory> {
        &self.memory
    }

    crate fn offset(&self) -> vk::DeviceSize {
        self.offset
    }

    crate fn size(&self) -> vk::DeviceSize {
        self.size
    }

    crate fn end(&self) -> vk::DeviceSize {
        self.offset + self.size
    }

    crate fn as_raw(&self) -> *mut c_void {
        if !self.memory.ptr.is_null() {
            unsafe { self.memory.ptr.add(self.offset as _) }
        } else { 0usize as _ }
    }

    crate fn as_slice<T: Sized>(&self) -> *mut [T] {
        let ptr = self.as_raw();
        assert_ne!(ptr, 0 as _);
        let mem_size = self.size as usize;
        let elem_size = std::mem::size_of::<T>();
        assert_eq!(mem_size % elem_size, 0);
        let slice_len = mem_size / elem_size;
        unsafe {
            std::slice::from_raw_parts_mut(ptr as *mut T, slice_len) as _
        }
    }


    crate fn as_ptr<T: Sized>(&self) -> *mut T {
        assert!(self.size() <= std::mem::size_of::<T>() as u64);
        self.as_raw() as _
    }

    fn block(&self) -> Block {
        Block {
            chunk: self.memory.chunk,
            start: self.offset,
            end: self.offset + self.size,
        }
    }
}

crate type DeviceAlloc = DeviceRange;

#[derive(Clone, Debug)]
crate struct DeviceBuffer {
    memory: Arc<DeviceMemory>,
    inner: vk::Buffer,
    usage: vk::BufferUsageFlags,
}

impl DeviceBuffer {
    crate fn memory(&self) -> &Arc<DeviceMemory> {
        &self.memory
    }

    crate fn inner(&self) -> vk::Buffer {
        self.inner
    }

    crate fn usage(&self) -> vk::BufferUsageFlags {
        self.usage
    }
}

/// A suballocation of a VkBuffer object.
#[derive(Clone, Debug)]
crate struct BufferSuballoc {
    buffer: Arc<DeviceBuffer>,
    offset: vk::DeviceSize,
    size: vk::DeviceSize,
    chunk: u32,
}

impl BufferSuballoc {
    crate fn buffer(&self) -> &Arc<DeviceBuffer> {
        &self.buffer
    }

    crate fn buffer_info(&self) -> vk::DescriptorBufferInfo {
        vk::DescriptorBufferInfo {
            buffer: self.buffer.inner,
            offset: self.offset,
            range: self.size,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Block {
    chunk: u32,
    start: vk::DeviceSize,
    end: vk::DeviceSize,
}

impl Block {
    fn offset(&self) -> vk::DeviceSize {
        self.start
    }

    fn size(&self) -> vk::DeviceSize {
        self.end - self.start
    }

    fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Address-ordered FIFO allocation algorithm.
#[derive(Debug, Default)]
struct FreeListAllocator {
    capacity: vk::DeviceSize,
    used: vk::DeviceSize,
    // List of chunk sizes
    chunks: Vec<vk::DeviceSize>,
    free: Vec<Block>,
}

impl FreeListAllocator {
    fn new() -> Self {
        Default::default()
    }

    fn used(&self) -> vk::DeviceSize {
        self.used
    }

    fn capacity(&self) -> vk::DeviceSize {
        self.capacity
    }

    fn add_chunk(&mut self, size: vk::DeviceSize) {
        self.capacity += size;
        self.chunks.push(size);
        self.free.push(Block {
            chunk: (self.chunks.len() - 1) as _,
            start: 0,
            end: size,
        });
    }

    fn carve_block(&mut self, index: usize, range: Range<vk::DeviceSize>) {
        self.used += range.end - range.start;

        let old_block = self.free[index];
        debug_assert!(old_block.start <= range.start &&
            range.end <= old_block.end);
        debug_assert!(range.start < range.end);

        // Resize/cull old block
        let mut block = &mut self.free[index];
        block.start = range.end;
        // TODO: Reverse free list order to prefer removal near end
        if block.is_empty() { self.free.remove(index); }

        // Insert padding block if necessary
        let chunk_idx = old_block.chunk;
        if range.start > old_block.start {
            let block = Block {
                chunk: chunk_idx,
                start: old_block.start,
                end: range.start,
            };
            self.free.insert(index, block);
        }
    }

    fn alloc_in(
        &mut self,
        block_idx: usize,
        size: vk::DeviceSize,
        alignment: vk::DeviceSize,
    ) -> Option<Block> {
        let block = &self.free[block_idx];
        let offset = align(alignment, block.start);
        if offset + size > block.end { return None; }
        let chunk = block.chunk;
        self.carve_block(block_idx, offset..offset + size);
        Some(Block {
            chunk,
            start: offset,
            end: offset + size,
        })
    }

    unsafe fn alloc(
        &mut self,
        size: vk::DeviceSize,
        alignment: vk::DeviceSize,
    ) -> Option<Block> {
        let aligned_size = align(alignment, size);
        let block = (0..self.free.len())
            .find_map(|block| self.alloc_in(block, aligned_size, alignment))?;
        Some(block)
    }

    fn free(&mut self, block: Block) {
        let chunk = block.chunk;
        let start = block.start;
        let end = block.end;

        self.used -= end - start;

        // Find insertion point
        // TODO: Binary search
        // TODO: If fragmentation is not an issue in practice, it might
        // not even be necessary to sort the free list
        let mut idx = self.free.len();
        for i in 0..self.free.len() {
            let block = self.free[i];
            if (block.chunk == chunk) & (start < block.start) {
                idx = i;
                break;
            }
        }

        // Detect adjacent blocks
        let merge_left = if idx > 0 {
            let left = self.free[idx - 1];
            assert!(left.chunk <= chunk);
            assert!((left.chunk < chunk) | (left.end <= start));
            (left.chunk == chunk) & (left.end == start)
        } else { false };
        let merge_right = if idx < self.free.len() {
            let right = self.free[idx];
            assert!(chunk <= right.chunk);
            assert!((chunk < right.chunk) | (end <= right.start));
            (right.chunk == chunk) & (end == right.start)
        } else { false };

        // Perform the insertion
        match (merge_left, merge_right) {
            (false, false) =>
                self.free.insert(idx, Block { chunk, start, end }),
            (true, false) => self.free[idx - 1].end = end,
            (false, true) => self.free[idx].start = start,
            (true, true) => {
                self.free[idx - 1].end = self.free[idx].end;
                self.free.remove(idx);
            },
        }
    }

    fn clear(&mut self) {
        self.free.clear();
        self.used = 0;
        for (i, &size) in self.chunks.iter().enumerate() {
            self.free.push(Block {
                chunk: i as _,
                start: 0,
                end: size,
            });
        }
    }
}

#[derive(Debug)]
crate struct HeapPool {
    device: Arc<Device>,
    type_index: u32,
    tiling: Tiling,
    allocator: FreeListAllocator,
    chunks: Vec<Arc<DeviceMemory>>,
}

impl Drop for HeapPool {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            for chunk in self.chunks.iter() {
                assert_eq!(Arc::strong_count(chunk), 1,
                    "allocation still in use: {:?}", chunk);
                dt.free_memory(chunk.inner, ptr::null());
            }
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
            inner,
            size,
            type_index: self.type_index,
            ptr: 0 as _,
            tiling: self.tiling,
            chunk,
        };
        if self.mapped() { mem.map(&self.device); }
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
        // Make sure the allocation comes from this pool
        assert!(Arc::ptr_eq(
            &alloc.memory,
            &self.chunks[alloc.memory.chunk as usize],
        ));
        self.allocator.free(alloc.block());
    }

    fn clear(&mut self) {
        self.allocator.clear()
    }
}

#[derive(Clone, Copy, Debug, Default)]
crate struct HeapInfo {
    reserved: vk::DeviceSize,
    used: vk::DeviceSize,
}

#[derive(Debug)]
crate struct DeviceHeap {
    device: Arc<Device>,
    // One pool per memory type per tiling
    pools: Vec<EnumMap<Tiling, HeapPool>>,
    heaps: Vec<HeapInfo>,
}

// TODO: dedicated allocations
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
        let heap_count = device.mem_props.memory_heap_count as usize;
        let heaps = vec![Default::default(); heap_count];
        DeviceHeap {
            device,
            pools,
            heaps,
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

    fn update_heaps(&mut self) {
        for heap in self.heaps.iter_mut() {
            heap.reserved = 0;
            heap.used = 0;
        }
        for pool in self.pools.iter().flat_map(|x| x.values()) {
            let heap = &mut self.heaps[pool.heap_index() as usize];
            heap.reserved += pool.reserved();
            heap.used += pool.used();
        }
    }

    crate fn heaps(&mut self) -> &[HeapInfo] {
        self.update_heaps();
        &self.heaps
    }

    crate unsafe fn alloc(
        &mut self,
        reqs: vk::MemoryRequirements,
        tiling: Tiling,
        mapped: bool,
    ) -> DeviceAlloc {
        let flags = if mapped {
            // TODO: cached incoherent memory
            vk::MemoryPropertyFlags::HOST_VISIBLE_BIT
                | vk::MemoryPropertyFlags::HOST_COHERENT_BIT
        } else {
            vk::MemoryPropertyFlags::DEVICE_LOCAL_BIT
        };

        // TODO: fall back to less restrictive flags on failure
        let type_idx = find_memory_type(
            &*self.device,
            flags,
            reqs.memory_type_bits,
        ).unwrap();

        self.pool_mut(type_idx, tiling).alloc(reqs.size, reqs.alignment)
    }

    /// Binds a buffer to newly allocated memory.
    crate unsafe fn alloc_buffer_memory(
        &mut self,
        buffer: vk::Buffer,
        mapped: bool,
    ) -> DeviceAlloc {
        let mut reqs = Default::default();
        self.dt().get_buffer_memory_requirements(buffer, &mut reqs);
        let alloc = self.alloc(reqs, Tiling::Linear, mapped);

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
        mapped: bool,
    ) -> DeviceAlloc {
        let mut reqs = Default::default();
        self.dt().get_image_memory_requirements(image, &mut reqs);
        let alloc = self.alloc(reqs, Tiling::Nonlinear, mapped);

        let memory = alloc.memory().inner();
        let offset = alloc.offset();
        self.dt().bind_image_memory(image, memory, offset).check().unwrap();

        alloc
    }

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

    unsafe fn device_heap_smoke(vars: testing::TestVars) {
        let device = Arc::clone(&vars.swapchain.device);
        let mut heap = DeviceHeap::new(device);

        let reqs = vk::MemoryRequirements {
            size: 4096,
            alignment: 256,
            memory_type_bits: !0,
        };
        let _alloc0 = heap.alloc(reqs, Tiling::Linear, true);
        let _alloc1 = heap.alloc(reqs, Tiling::Nonlinear, false);
        assert_ne!(_alloc0.as_raw(), 0 as _);
        heap.free(_alloc0);
    }

    unit::declare_tests![
        device_heap_smoke,
    ];
}

unit::collect_tests![tests];
