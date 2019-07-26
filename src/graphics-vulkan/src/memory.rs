//! This module defines memory allocators. It is not responsible for
//! populating memory, i.e. resource management.
use std::ops::Range;
use std::ffi::c_void;
use std::ptr;
use std::sync::Arc;

use crate::*;

#[inline(always)]
crate fn visible_coherent_memory() -> vk::MemoryPropertyFlags {
    vk::MemoryPropertyFlags::HOST_VISIBLE_BIT |
        vk::MemoryPropertyFlags::HOST_COHERENT_BIT
}

/// This struct includes the information about an allocation that is
/// relevant to users of that memory. It includes a size and location
/// within device memory as well as an optional pointer to a region of
/// host address space mapped to said memory. It doesn't include
/// allocator-specific metadata and can't be used to correctly free most
/// allocations.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DeviceSlice {
    pub memory: vk::DeviceMemory,
    pub offset: vk::DeviceSize,
    pub size: vk::DeviceSize,
    pub ptr: *mut c_void,
}

impl Default for DeviceSlice {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl DeviceSlice {
    pub fn end(&self) -> vk::DeviceSize {
        self.offset + self.size
    }

    pub fn as_slice<T: Sized>(&self) -> *mut [T] {
        let mem_size = self.size as usize;
        let elem_size = std::mem::size_of::<T>();
        assert_eq!(mem_size % elem_size, 0);
        let slice_len = mem_size / elem_size;
        unsafe {
            std::slice::from_raw_parts_mut(self.ptr as *mut T, slice_len) as _
        }
    }

    #[allow(dead_code)]
    pub fn as_block<T: Sized>(&self) -> *mut T {
        assert_eq!(self.size as usize, std::mem::size_of::<T>());
        self.ptr as _
    }
}

/// Returns whether the type index is in the bitmask of types.
fn compatible_type(type_bits: u32, type_index: u32) -> bool {
    type_bits & (1 << type_index) > 0
}

pub fn iter_memory_types(props: &vk::PhysicalDeviceMemoryProperties) ->
    impl Iterator<Item = &vk::MemoryType>
{
    props.memory_types.iter().take(props.memory_type_count as _)
}

/// Finds a desirable memory type that meets requirements. This
/// function follows the guidelines in the Vulkan spec stating that
/// implementations are to sort memory types in order of "performance",
/// so the first memory type with the required properties is probably
/// the best for general use.
pub fn find_memory_type(device: &Device, flags: vk::MemoryPropertyFlags) ->
    Option<u32>
{
    iter_memory_types(&device.mem_props)
        .position(|ty| ty.property_flags.contains(flags))
        .map(|x| x as _)
}

#[derive(Clone, Copy, Debug)]
pub struct CommonAlloc {
    info: DeviceSlice,
    // Allocator-specific data (e.g. array indices and flags).
    data_0: u32,
    data_1: u32,
}

impl CommonAlloc {
    pub fn info(&self) -> &DeviceSlice {
        &self.info
    }
}

// TODO: dedicated allocations
// TODO: I think this trait should actually be a wrapper type and the
// underlying trait should only implement alloc and free
crate trait DeviceAllocator {
    fn dt(&self) -> &vkl::DeviceTable;

    /// Allocates a chunk of device memory without knowledge of its
    /// future contents.
    unsafe fn allocate(&mut self, reqs: vk::MemoryRequirements) -> CommonAlloc;

    /// Frees a memory allocation, if possible. If any resource is still
    /// bound to that memory, it may alias a future allocation that
    /// overlaps the same range.
    unsafe fn free(&mut self, alloc: &CommonAlloc);

    /// Binds a buffer to newly allocated memory.
    unsafe fn alloc_buffer_memory(&mut self, buffer: vk::Buffer) -> CommonAlloc
    {
        let mut reqs = Default::default();
        self.dt().get_buffer_memory_requirements(buffer, &mut reqs as _);
        let alloc = self.allocate(reqs);

        let DeviceSlice { memory, offset, .. } = *alloc.info();
        self.dt().bind_buffer_memory(buffer, memory, offset)
            .check().unwrap();

        alloc
    }

    /// Binds an image to newly allocated memory.
    unsafe fn alloc_image_memory(&mut self, image: vk::Image) -> CommonAlloc {
        let mut reqs = Default::default();
        self.dt().get_image_memory_requirements(image, &mut reqs as _);
        let alloc = self.allocate(reqs);

        let DeviceSlice { memory, offset, .. } = *alloc.info();
        self.dt().bind_image_memory(image, memory, offset)
            .check().unwrap();

        alloc
    }
}

/// This is a simple address-ordered FIFO allocator. It is somewhat
/// low-level as it doesn't check for correct memory usage.
#[derive(Debug)]
pub struct MemoryPool {
    device: Arc<Device>,
    type_index: u32,
    mapped: bool,
    base_size: vk::DeviceSize,
    chunks: Vec<Chunk>,
    free: Vec<FreeBlock>,
}

impl Drop for MemoryPool {
    fn drop(&mut self) {
        for &chunk in self.chunks.iter() {
            unsafe { self.dt().free_memory(chunk.memory, ptr::null()); }
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MemoryPoolCreateInfo {
    pub type_index: u32,
    pub mapped: bool,
    pub base_size: vk::DeviceSize,
}

#[derive(Clone, Copy, Debug)]
struct Chunk {
    memory: vk::DeviceMemory,
    size: vk::DeviceSize,
    ptr: *mut c_void,
}

#[derive(Clone, Copy, Debug, Default)]
struct FreeBlock {
    chunk: usize,
    start: vk::DeviceSize,
    end: vk::DeviceSize,
}

impl FreeBlock {
    fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

impl MemoryPool {
    pub unsafe fn new(
        device: Arc<Device>,
        create_info: MemoryPoolCreateInfo,
    ) -> Self {
        let mut res = MemoryPool {
            device,
            type_index: create_info.type_index,
            mapped: create_info.mapped,
            base_size: create_info.base_size,
            chunks: Vec::new(),
            free: Vec::new(),
        };
        assert!(!res.mapped() ||
            res.flags().contains(vk::MemoryPropertyFlags::HOST_VISIBLE_BIT));
        res.grow(create_info.base_size);
        res
    }

    pub unsafe fn grow(&mut self, size: vk::DeviceSize) {
        let dt = &self.dt();

        let alloc_info = vk::MemoryAllocateInfo {
            allocation_size: size,
            memory_type_index: self.type_index,
            ..Default::default()
        };
        let mut memory = vk::null();
        dt.allocate_memory(&alloc_info, ptr::null(), &mut memory as _)
            .check().expect("failed to allocate device memory");

        let mut ptr = 0usize as *mut c_void;
        if self.mapped {
            let flags = Default::default();
            dt.map_memory(memory, 0, size, flags, &mut ptr as _)
                .check().expect("failed to map device memory");
        }

        self.chunks.push(Chunk { memory, size, ptr });
        self.free.push(FreeBlock {
            chunk: self.chunks.len() - 1,
            start: 0,
            end: size,
        });
    }

    unsafe fn carve_block(
        &mut self,
        index: usize,
        range: Range<vk::DeviceSize>,
    ) -> CommonAlloc {
        let old_block = self.free[index];
        debug_assert!(old_block.start <= range.start &&
            range.end <= old_block.end);
        debug_assert!(range.start < range.end);

        // Resize/cull old block
        let mut block = &mut self.free[index];
        block.start = range.end;
        if block.is_empty() { self.free.remove(index); }

        // Insert padding block if necessary
        let chunk_idx = old_block.chunk;
        if range.start > old_block.start {
            self.free.insert(index, FreeBlock {
                chunk: chunk_idx,
                start: old_block.start,
                end: range.start,
            });
        }

        let chunk = self.chunks[chunk_idx];
        let ptr = if !chunk.ptr.is_null() { chunk.ptr.add(range.start as _) }
            else { 0usize as _ };
        CommonAlloc {
            info: DeviceSlice {
                memory: chunk.memory,
                offset: range.start,
                size: range.end - range.start,
                ptr,
            },
            data_0: chunk_idx as _,
            data_1: 0,
        }
    }

    pub fn mapped(&self) -> bool {
        self.mapped
    }

    pub fn flags(&self) -> vk::MemoryPropertyFlags {
        self.device.mem_props.memory_types[self.type_index as usize]
            .property_flags
    }
}


impl DeviceAllocator for MemoryPool {
    fn dt(&self) -> &vkl::DeviceTable {
        &self.device.table
    }

    unsafe fn allocate(&mut self, reqs: vk::MemoryRequirements) -> CommonAlloc
    {
        assert!(compatible_type(reqs.memory_type_bits, self.type_index));
        // Avoid leaving padding bytes as free blocks
        let size = align_64(reqs.alignment, reqs.size);

        for idx in 0..self.free.len() {
            let block = &mut self.free[idx];
            let offset = align_64(reqs.alignment, block.start);
            if block.end - offset >= size {
                // Found a spot
                return self.carve_block(idx, offset..(offset + size));
            }
        }

        // Didn't find a block; allocate a new chunk and put the
        // allocation there.
        self.grow(align_64(self.base_size, size));

        let block = self.free.len() - 1;
        self.carve_block(block, 0..size)
    }

    unsafe fn free(&mut self, alloc: &CommonAlloc) {
        let chunk = alloc.data_0 as usize;
        assert!(chunk < self.chunks.len());
        let info = alloc.info;
        let (start, end) = (info.offset, info.end());

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
}
