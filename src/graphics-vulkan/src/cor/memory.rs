use std::ops::Range;
use std::ffi::c_void;
use std::ptr;
use std::sync::Arc;

use prelude::*;

use crate::*;

#[inline(always)]
crate fn visible_coherent_memory() -> vk::MemoryPropertyFlags {
    vk::MemoryPropertyFlags::HOST_VISIBLE_BIT |
        vk::MemoryPropertyFlags::HOST_COHERENT_BIT
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
crate struct AllocInfo {
    crate memory: vk::DeviceMemory,
    crate offset: vk::DeviceSize,
    crate size: vk::DeviceSize,
    /// Buffer containing the sub-allocation, if available.
    crate buffer: vk::Buffer,
    /// Offset into the buffer object.
    crate buf_offset: vk::DeviceSize,
    /// Memory-mapped pointer, if available.
    crate ptr: *mut c_void,
}

impl Default for AllocInfo {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl AllocInfo {
    crate fn as_slice<T: Sized>(&self) -> *mut [T] {
        let mem_size = self.size as usize;
        let elem_size = std::mem::size_of::<T>();
        assert_eq!(mem_size % elem_size, 0);
        let slice_len = mem_size / elem_size;
        unsafe {
            std::slice::from_raw_parts_mut(self.ptr as *mut T, slice_len) as _
        }
    }

    crate fn as_block<T: Sized>(&self) -> *mut T {
        self.ptr as _
    }

    crate fn end(&self) -> vk::DeviceSize {
        self.offset + self.size
    }

    crate fn buffer_info(&self) -> vk::DescriptorBufferInfo {
        vk::DescriptorBufferInfo {
            buffer: self.buffer,
            offset: self.offset,
            range: self.size,
        }
    }
}

/// Returns whether the type index is in the bitmask of types.
fn compatible_type(type_bits: u32, type_index: u32) -> bool {
    type_bits & (1 << type_index) > 0
}

crate fn iter_memory_types(props: &vk::PhysicalDeviceMemoryProperties) ->
    impl Iterator<Item = &vk::MemoryType>
{
    props.memory_types.iter().take(props.memory_type_count as _)
}

/// Finds a desirable memory type that meets requirements. This
/// function follows the guidelines in the Vulkan spec stating that
/// implementations are to sort memory types in order of "performance",
/// so the first memory type with the required properties is probably
/// the best for general use.
crate fn find_memory_type(device: &Device, flags: vk::MemoryPropertyFlags) ->
    Option<u32>
{
    iter_memory_types(&device.mem_props)
        .position(|ty| ty.property_flags.contains(flags))
        .map(|x| x as _)
}

#[derive(Clone, Copy, Debug)]
crate struct DeviceAlloc {
    info: AllocInfo,
    chunk_idx: u32,
}

macro_rules! delegate {
    ($parent:ident, $child:ident, {$($field:ident: $type:ty),*$(,)*}) => {
        impl $parent {
            $(
                crate fn $field(&self) -> $type {
                    self.$child.$field
                }
            )*
        }
    }
}

delegate!(DeviceAlloc, info, {
    memory: vk::DeviceMemory,
    offset: vk::DeviceSize,
    size: vk::DeviceSize,
    buffer: vk::Buffer,
    buf_offset: vk::DeviceSize,
    ptr: *mut c_void,
});

impl DeviceAlloc {
    crate fn info(&self) -> &AllocInfo {
        &self.info
    }

    crate fn as_slice<T: Sized>(&self) -> *mut [T] {
        self.info.as_slice()
    }

    crate fn as_block<T: Sized>(&self) -> *mut T {
        self.info.as_block()
    }

    crate fn end(&self) -> vk::DeviceSize {
        self.info.end()
    }

    crate fn buffer_info(&self) -> vk::DescriptorBufferInfo {
        self.info.buffer_info()
    }
}

/// This is a simple address-ordered FIFO allocator. It is somewhat
/// low-level as it doesn't check for correct memory usage (i.e.
/// linear/non-linear overlap).
// TODO: Maybe store a map from allocation size to free blocks.
// TODO: Stack-like allocation
//
#[derive(Debug)]
crate struct MemoryPool {
    device: Arc<Device>,
    type_index: u32,
    host_mapped: bool,
    buffer_map: Option<BufferMapOptions>,
    base_size: vk::DeviceSize,
    capacity: vk::DeviceSize,
    used: vk::DeviceSize,
    chunks: Vec<Chunk>,
    free: Vec<FreeBlock>,
}

impl Drop for MemoryPool {
    fn drop(&mut self) {
        for &chunk in self.chunks.iter() {
            unsafe {
                self.dt().destroy_buffer(chunk.buffer, ptr::null());
                self.dt().free_memory(chunk.memory, ptr::null());
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
crate struct BufferMapOptions {
    crate usage: vk::BufferUsageFlags,
}

#[derive(Clone, Copy, Debug, Default)]
crate struct MemoryPoolCreateInfo {
    crate type_index: u32,
    crate base_size: vk::DeviceSize,
    /// Map all memory to host address space. Requires host visible
    /// memory.
    crate host_mapped: bool,
    /// If provided, wraps all memory in a VkBuffer. Allocations will
    /// alias a region of one of these buffers.
    crate buffer_map: Option<BufferMapOptions>,
}

#[derive(Clone, Copy, Debug)]
struct Chunk {
    memory: vk::DeviceMemory,
    buffer: vk::Buffer,
    size: vk::DeviceSize,
    ptr: *mut c_void,
}

#[derive(Clone, Copy, Debug, Default)]
struct FreeBlock {
    chunk: u32,
    start: vk::DeviceSize,
    end: vk::DeviceSize,
}

impl FreeBlock {
    fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

impl MemoryPool {
    crate unsafe fn new(
        device: Arc<Device>,
        create_info: MemoryPoolCreateInfo,
    ) -> Self {
        let mem = MemoryPool {
            device,
            type_index: create_info.type_index,
            host_mapped: create_info.host_mapped,
            buffer_map: create_info.buffer_map,
            base_size: create_info.base_size,
            capacity: 0,
            used: 0,
            chunks: Vec::new(),
            free: Vec::new(),
        };
        println!();
        assert!(mem.type_index < mem.device.mem_props.memory_type_count);
        assert!(!mem.host_mapped() ||
            mem.flags().contains(vk::MemoryPropertyFlags::HOST_VISIBLE_BIT));
        mem
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    // Guarantees that each allocation is aligned to `quantum` bytes and
    // is a multiple of `quantum` in length. This is transparent to the
    // user, who sees exactly the size they requested.
    fn quantum(&self) -> vk::DeviceSize {
        // sizeof(vec4) seems like a good choice
        16
    }

    crate fn host_mapped(&self) -> bool {
        self.host_mapped
    }

    crate fn buffer_mapped(&self) -> bool {
        self.buffer_map.is_some()
    }

    crate fn buffer_map(&self) -> Option<&BufferMapOptions> {
        self.buffer_map.as_ref()
    }

    crate fn flags(&self) -> vk::MemoryPropertyFlags {
        self.device.mem_props.memory_types[self.type_index as usize]
            .property_flags
    }

    crate fn used(&self) -> vk::DeviceSize {
        self.used
    }

    crate fn capacity(&self) -> vk::DeviceSize {
        self.capacity
    }

    crate unsafe fn grow(&mut self, size: vk::DeviceSize) {
        let dt = self.dt();

        let alloc_info = vk::MemoryAllocateInfo {
            allocation_size: size,
            memory_type_index: self.type_index,
            ..Default::default()
        };
        let mut memory = vk::null();
        dt.allocate_memory(&alloc_info, ptr::null(), &mut memory)
            .check().expect("failed to allocate device memory");

        let mut ptr = 0usize as *mut c_void;
        if self.host_mapped {
            let flags = Default::default();
            dt.map_memory(memory, 0, size, flags, &mut ptr)
                .check().expect("failed to map device memory");
        }

        let mut buffer = vk::null();
        if let Some(ref opts) = &self.buffer_map {
            let create_info = vk::BufferCreateInfo {
                size,
                usage: opts.usage,
                ..Default::default()
            };
            dt.create_buffer(&create_info, ptr::null(), &mut buffer)
                .check().unwrap();

            let mut reqs = vk::MemoryRequirements::default();
            dt.get_buffer_memory_requirements(buffer, &mut reqs);
            assert_eq!(reqs.size, size);
            assert!(
                compatible_type(reqs.memory_type_bits, self.type_index),
                "type_index: {}, memory_type_bits: 0b{:b}",
                self.type_index, reqs.memory_type_bits,
            );

            dt.bind_buffer_memory(buffer, memory, 0).check().unwrap();
        }

        self.chunks.push(Chunk { memory, buffer, size, ptr });
        self.free.push(FreeBlock {
            chunk: (self.chunks.len() - 1) as _,
            start: 0,
            end: size,
        });

        self.capacity += size;
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
        // TODO: Reverse free list order so we remove from the end
        // rather than the beginning.
        if block.is_empty() { self.free.remove(index); }

        // Insert padding block if necessary
        let chunk_idx = old_block.chunk;
        if range.start > old_block.start {
            let block = FreeBlock {
                chunk: chunk_idx,
                start: old_block.start,
                end: range.start,
            };
            self.free.insert(index, block);
        }
    }

    fn try_alloc(
        &mut self,
        idx: usize,
        size: vk::DeviceSize,
        alignment: vk::DeviceSize,
    ) -> Option<(u32, vk::DeviceSize)> {
        let block = &self.free[idx];
        let offset = align(alignment, block.start);
        if offset + size > block.end { return None; }
        let chunk = block.chunk;
        self.carve_block(idx, offset..offset + size);
        Some((chunk, offset))
    }

    /// Allocates a chunk of memory without binding a resource to it.
    crate unsafe fn allocate(
        &mut self,
        size: vk::DeviceSize,
        alignment: vk::DeviceSize,
    ) -> DeviceAlloc {
        // TODO: make alignment comply with
        // minStorageBufferOffsetAlignment etc.
        let alignment = std::cmp::max(alignment, self.quantum());
        let padded_size = align(size, self.quantum());
        let (chunk_idx, offset) = (0..self.free.len())
            .find_map(|idx| self.try_alloc(idx, padded_size, alignment))
            .or_else(|| {
                // Didn't find a block; allocate a new chunk and put the
                // allocation there.
                self.grow(align(self.base_size, padded_size));
                self.try_alloc(self.free.len() - 1, padded_size, alignment)
            })
            .unwrap();
        let chunk = self.chunks[chunk_idx as usize];
        let ptr = if !chunk.ptr.is_null() { chunk.ptr.add(offset as _) }
            else { 0usize as _ };
        DeviceAlloc {
            info: AllocInfo {
                memory: chunk.memory,
                offset,
                size,
                buffer: chunk.buffer,
                buf_offset: offset,
                ptr,
            },
            chunk_idx,
        }
    }

    crate unsafe fn alloc_with_reqs(&mut self, reqs: vk::MemoryRequirements) ->
        DeviceAlloc
    {
        assert!(compatible_type(reqs.memory_type_bits, self.type_index));
        self.allocate(reqs.size, reqs.alignment)
    }

    /// Frees a memory allocation, if possible. If any resource is still
    /// bound to that memory, it may alias a future allocation that
    /// overlaps the same range.
    crate unsafe fn free(&mut self, alloc: DeviceAlloc) {
        let chunk = alloc.chunk_idx;
        assert_eq!(self.chunks[chunk as usize].memory, alloc.memory());
        let info = alloc.info();
        let start = info.offset;
        let end = align(self.quantum(), info.end());

        self.used -= end - start;

        // TODO: Optimize search and insertion
        let mut idx = self.free.len();
        for i in 0..self.free.len() {
            let block = self.free[i];
            if (block.chunk == chunk) & (start < block.start) {
                idx = i;
                break;
            }
        }

        // Found insertion point
        let merge_left = if idx > 0 {
            let left = self.free[idx - 1];
            (left.chunk == chunk) & (left.end == start)
        } else { false };
        let merge_right = if idx < self.free.len() {
            let right = self.free[idx];
            (right.chunk == chunk) & (end == right.start)
        } else { false };

        match (merge_left, merge_right) {
            (false, false) =>
                self.free.insert(idx, FreeBlock {
                    chunk,
                    start,
                    end,
                }),
            (true, false) => self.free[idx - 1].end = end,
            (false, true) => self.free[idx].start = start,
            (true, true) => {
                self.free[idx - 1].end = self.free[idx].end;
                self.free.remove(idx);
            },
        }
    }

    crate fn clear(&mut self) {
        self.free.clear();
        self.used = 0;
        for (i, chunk) in self.chunks.iter().enumerate() {
            self.free.push(FreeBlock {
                chunk: i as _,
                start: 0,
                end: chunk.size,
            });
        }
    }

    fn dt(&self) -> &vkl::DeviceTable {
        &self.device.table
    }

    // TODO: dedicated allocations
    /// Binds a buffer to newly allocated memory.
    crate unsafe fn alloc_buffer_memory(&mut self, buffer: vk::Buffer) ->
        DeviceAlloc
    {
        let mut reqs = Default::default();
        self.dt().get_buffer_memory_requirements(buffer, &mut reqs);
        let alloc = self.alloc_with_reqs(reqs);

        let &AllocInfo { memory, offset, .. } = alloc.info();
        self.dt().bind_buffer_memory(buffer, memory, offset).check().unwrap();

        alloc
    }

    // TODO: dedicated allocations
    /// Binds an image to newly allocated memory.
    crate unsafe fn alloc_image_memory(&mut self, image: vk::Image) ->
        DeviceAlloc
    {
        let mut reqs = Default::default();
        self.dt().get_image_memory_requirements(image, &mut reqs);
        let alloc = self.alloc_with_reqs(reqs);

        let &AllocInfo { memory, offset, .. } = alloc.info();
        self.dt().bind_image_memory(image, memory, offset).check().unwrap();

        alloc
    }
}

#[cfg(test)]
mod tests {
    use vk::traits::*;
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let device = Arc::clone(&vars.swapchain.device);

        let flags = vk::MemoryPropertyFlags::DEVICE_LOCAL_BIT;
        let type_index = find_memory_type(&device, flags).unwrap();
        let create_info = MemoryPoolCreateInfo {
            type_index,
            base_size: 0x100_0000,
            ..Default::default()
        };
        let mut memory = MemoryPool::new(device, create_info);

        let alloc0 = memory.allocate(0x1000, 0x100);
        let alloc1 = memory.allocate(0x1000, 0x100);

        assert!(memory.capacity() >= 0x2000);

        let info0 = alloc0.info();
        let info1 = alloc1.info();

        assert!(!info0.memory.is_null());
        assert_eq!(info0.memory, info1.memory);

        assert_eq!(info0.size, 0x1000);
        assert_eq!(info1.size, info0.size);

        assert_eq!(info0.offset, 0);
        assert_eq!(info1.offset, info0.size);

        assert!(info0.ptr.is_null());
        assert!(info0.buffer.is_null());

        assert_eq!(memory.used(), 0x2000);
        memory.free(alloc0);
        assert_eq!(memory.used(), 0x1000);
        memory.free(alloc1);
        assert_eq!(memory.used(), 0);

        memory.allocate(0x1000, 0x100);
        memory.allocate(0x1000, 0x100);
        assert_eq!(memory.used(), 0x2000);
        memory.clear();
        assert_eq!(memory.used(), 0);
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
