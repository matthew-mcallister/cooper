//! This module defines memory allocators. It is not responsible for
//! populating memory or binding buffers, as that is the resource
//! manager's job.
use std::ops::Range;
use std::ffi::c_void;
use std::ptr;
use std::sync::Arc;

use crate::*;

#[inline]
fn align_to(alignment: vk::DeviceSize, offset: vk::DeviceSize) ->
    vk::DeviceSize
{
    ((offset + alignment - 1) / alignment) * alignment
}

/// This struct includes information about an allocation that is
/// relevant to users of that memory. It doesn't include
/// allocator-specific metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
crate struct AllocInfo {
    crate memory: vk::DeviceMemory,
    crate offset: vk::DeviceSize,
    crate size: vk::DeviceSize,
    crate ptr: *mut c_void,
}

impl Default for AllocInfo {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl AllocInfo {
    crate fn end(&self) -> vk::DeviceSize {
        self.offset + self.size
    }
}

/// Returns whether the type index is in the bitmask of types.
crate fn compatible_type(type_bits: u32, type_index: u32) -> bool {
    type_bits & (1 << type_index) > 0
}

/// Finds a desirable memory type that meets requirements. This
/// method follows the guidelines in the spec stating that
/// implementations are to sort memory types in order of performance, so
/// the first memory type with the required properties is probably the
/// best for general use.
crate fn find_type_index(
    props: &vk::PhysicalDeviceMemoryProperties,
    type_bits: u32,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    for idx in 0..props.memory_type_count {
        let f = props.memory_types[idx as usize].property_flags;
        if compatible_type(type_bits, idx) && flags.contains(f)
            { return Some(idx); }
    }
    None
}

/// The result of allocating memory through an allocator.
#[derive(Clone, Copy, Debug)]
crate struct CommonAlloc {
    info: AllocInfo,
    // Allocator-specific data (e.g. array indices and flags).
    data_0: u32,
    data_1: u32,
}

impl CommonAlloc {
    /// Returns relevant info for the user of the memory.
    crate fn info(&self) -> &AllocInfo {
        &self.info
    }
}

/// This is a simple address-ordered FIFO allocator. It is somewhat
/// low-level as it doesn't check for correct memory usage.
#[derive(Debug)]
crate struct MemoryPool {
    dt: Arc<vkl::DeviceTable>,
    type_index: u32,
    map_memory: bool,
    chunks: Vec<Chunk>,
    free: Vec<FreeBlock>,
}

impl Drop for MemoryPool {
    fn drop(&mut self) {
        for &chunk in self.chunks.iter() {
            unsafe { self.dt.free_memory(chunk.memory, ptr::null()); }
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
crate struct MemoryPoolCreateInfo {
    crate type_index: u32,
    crate map_memory: bool,
    crate capacity: vk::DeviceSize,
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
    crate unsafe fn new(
        dt: Arc<vkl::DeviceTable>,
        create_info: MemoryPoolCreateInfo,
    ) -> Self {
        let mut res = MemoryPool {
            dt,
            type_index: create_info.type_index,
            map_memory: create_info.map_memory,
            chunks: Vec::new(),
            free: Vec::new(),
        };
        res.grow(create_info.capacity);
        res
    }

    crate unsafe fn grow(&mut self, size: vk::DeviceSize) {
        let alloc_info = vk::MemoryAllocateInfo {
            allocation_size: size,
            memory_type_index: self.type_index,
            ..Default::default()
        };
        let mut memory = vk::null();
        self.dt.allocate_memory(&alloc_info, ptr::null(), &mut memory as _)
            .check().expect("failed to allocate device memory");

        let mut ptr = 0usize as *mut c_void;
        if self.map_memory {
            let flags = Default::default();
            self.dt.map_memory(memory, 0, size, flags, &mut ptr as _)
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
            info: AllocInfo {
                memory: chunk.memory,
                offset: range.start,
                size: range.end - range.start,
                ptr,
            },
            data_0: chunk_idx as _,
            data_1: 0,
        }
    }

    crate unsafe fn allocate(&mut self, reqs: vk::MemoryRequirements) ->
        CommonAlloc
    {
        // TODO: dedicated allocations (mid-priority)
        assert!(compatible_type(reqs.memory_type_bits, self.type_index));
        // I don't know what this accomplishes besides reducing padding.
        let size = align_to(reqs.alignment, reqs.size);

        for idx in 0..self.free.len() {
            let block = &mut self.free[idx];
            let offset = align_to(reqs.alignment, block.start);
            if block.end - offset >= size {
                // Found a spot
                return self.carve_block(idx, offset..(offset + size));
            }
        }

        // Didn't find a block; allocate a new chunk and put the
        // allocation there.
        // TODO: optimize growth formula
        let grow_size = self.chunks.last().unwrap().size;
        self.grow(std::cmp::max(grow_size, size));

        let block = self.free.len() - 1;
        self.carve_block(block, 0..size)
    }

    /// Frees an allocation of memory. If any resource is still bound to
    /// that memory, it may alias a future allocation at that site.
    crate unsafe fn free(&mut self, alloc: &CommonAlloc) {
        let chunk = alloc.data_0 as usize;
        assert!(chunk < self.chunks.len());
        let info = alloc.info;
        let (start, end) = (info.offset, info.end());

        // TODO: Optimize search and insertion (b-tree maybe)
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
                self.free[idx].start = self.free[idx - 1].start;
                self.free.remove(idx - 1);
            },
        }
    }

    // TODO: reserve this for ResourceAllocator, as this class is meant
    // to be low level
    /// Creates a buffer and immediately binds it to memory.
    crate unsafe fn create_buffer(
        &mut self,
        create_info: &vk::BufferCreateInfo,
    ) -> (vk::Buffer, CommonAlloc) {
        let mut buf = vk::null();
        self.dt.create_buffer(create_info as _, ptr::null(), &mut buf as _)
            .check().unwrap();

        let mut reqs = Default::default();
        self.dt.get_buffer_memory_requirements(buf, &mut reqs as _);
        let alloc = self.allocate(reqs);

        self.dt.bind_buffer_memory(buf, alloc.info.memory, alloc.info.offset)
            .check().unwrap();

        (buf, alloc)
    }

    /// Creates an image and immediately binds it to memory.
    crate unsafe fn create_image(&mut self, create_info: &vk::ImageCreateInfo)
        -> (vk::Image, CommonAlloc)
    {
        let mut image = vk::null();
        self.dt.create_image(create_info as _, ptr::null(), &mut image as _)
            .check().unwrap();

        let mut reqs = Default::default();
        self.dt.get_image_memory_requirements(image, &mut reqs as _);
        let alloc = self.allocate(reqs);

        self.dt.bind_image_memory(image, alloc.info.memory, alloc.info.offset)
            .check().unwrap();

        (image, alloc)
    }
}

/// A flexible allocator that can create and free buffers and images
/// dynamically. It is mostly intended to provide storage for static
/// data in device-local memory, e.g. vertex buffers and textures.
// TODO: implement this
#[derive(Debug)]
crate struct ResourceAllocator {
    // Buffer-image granularity is funky, so we sidestep the issue by
    // allocating linear and non-linear resources from separate pools.
    linear_pool: MemoryPool,
    nonlinear_pool: MemoryPool,
}
