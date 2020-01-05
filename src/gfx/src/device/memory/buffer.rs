use std::ops::Range;
use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::ptr::{self, NonNull};
use std::sync::Arc;

use enum_map::{Enum, EnumMap};
use prelude::*;

use crate::*;
use super::*;

#[derive(Clone, Debug)]
crate struct DeviceBuffer {
    memory: Arc<DeviceMemory>,
    inner: vk::Buffer,
    usage: vk::BufferUsageFlags,
    binding: BufferBinding,
    mapping: MemoryMapping,
}

/// A suballocation of a VkBuffer object.
#[derive(Clone, Debug)]
crate struct BufferRange {
    buffer: Arc<DeviceBuffer>,
    offset: vk::DeviceSize,
    // N.B. the allocator might return more memory than requested.
    size: vk::DeviceSize,
}

#[derive(Debug)]
crate struct BufferBox<T: ?Sized> {
    alloc: BufferRange,
    ptr: NonNull<T>,
}

impl Drop for DeviceBuffer {
    fn drop(&mut self) {
        let dt = &*self.device().table;
        unsafe { dt.destroy_buffer(self.inner, ptr::null()); }
    }
}

impl DeviceBuffer {
    crate fn device(&self) -> &Arc<Device> {
        &self.memory.device
    }

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

impl MemoryRegion for BufferRange {
    fn memory(&self) -> &Arc<DeviceMemory> {
        &self.buffer.memory
    }

    fn offset(&self) -> vk::DeviceSize {
        self.offset
    }

    fn size(&self) -> vk::DeviceSize {
        self.size
    }
}

impl BufferRange {
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

    crate fn chunk(&self) -> u32 {
        self.buffer.memory.chunk
    }
}

impl<T: ?Sized> Drop for BufferBox<T> {
    fn drop(&mut self) {
        unsafe { std::ptr::drop_in_place(self.ptr.as_ptr()); }
    }
}

impl<T: ?Sized> From<BufferBox<T>> for BufferRange {
    fn from(data: BufferBox<T>) -> Self {
        data.into_inner()
    }
}

impl<T: ?Sized> BufferBox<T> {
    unsafe fn new(alloc: BufferRange, ptr: *mut T) -> Self {
        BufferBox { alloc, ptr: NonNull::new_unchecked(ptr) }
    }

    crate fn alloc(&self) -> &BufferRange {
        &self.alloc
    }

    crate fn into_inner(self) -> BufferRange {
        unsafe {
            std::ptr::drop_in_place(self.ptr.as_ptr());
            let alloc = ptr::read(&self.alloc);
            std::mem::forget(self);
            alloc
        }
    }
}

impl<T> BufferBox<T> {
    crate fn from_val(mut alloc: BufferRange, val: T) -> Self {
        let ptr = alloc.as_mut::<T>().write(val) as _;
        unsafe { BufferBox::new(alloc, ptr) }
    }
}

impl<T> BufferBox<[T]> {
    fn from_iter(
        mut alloc: BufferRange,
        iter: impl Iterator<Item = T> + ExactSizeIterator,
    ) -> Self {
        let slice = alloc.as_mut_slice::<T>(iter.len());
        for (dst, src) in slice.iter_mut().zip(iter) {
            dst.write(src);
        }
        let ptr = slice as *mut _ as _;
        unsafe { BufferBox::new(alloc, ptr) }
    }
}

impl<T: Copy> BufferBox<[T]> {
    crate fn copy_from_slice(mut alloc: BufferRange, src: &[T]) -> Self {
        let slice = alloc.as_mut_slice::<T>(src.len());
        slice.copy_from_slice(as_uninit_slice(src));
        let slice = slice as *mut _ as _;
        unsafe { BufferBox::new(alloc, slice) }
    }
}

impl<T: ?Sized> std::ops::Deref for BufferBox<T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}


impl<T: ?Sized> std::ops::DerefMut for BufferBox<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ptr.as_mut() }
    }
}

#[derive(Debug)]
crate struct BufferHeap {
    device: Arc<Device>,
    pools: EnumMap<BufferBinding, BufferHeapEntry>,
}

#[derive(Debug)]
struct BufferHeapEntry {
    binding: BufferBinding,
    // Memory mapped pool and the only pool on UMA.
    mapped_pool: BufferPool,
    // Unmapped, non-host-visible pool on discrete systems.
    unmapped_pool: Option<BufferPool>,
}

#[derive(Debug)]
struct BufferPool {
    device: Arc<Device>,
    binding: BufferBinding,
    mapping: MemoryMapping,
    allocator: FreeListAllocator,
    chunks: Vec<Arc<DeviceBuffer>>,
}

#[derive(Clone, Copy, Debug, Enum, Eq, Hash, PartialEq)]
crate enum BufferBinding {
    Storage,
    Uniform,
    StorageTexel,
    UniformTexel,
    Vertex,
    Index,
}

impl BufferHeap {
    crate fn new(device: Arc<Device>) -> Self {
        let pools = (|binding| BufferHeapEntry::new(&device, binding)).into();
        BufferHeap {
            device,
            pools,
        }
    }

    crate fn alloc(
        &mut self,
        binding: BufferBinding,
        mapping: MemoryMapping,
        size: vk::DeviceSize,
    ) -> BufferRange {
        self.pools[binding].alloc(mapping, size)
    }

    crate fn free(&mut self, alloc: impl Into<BufferRange>) {
        let alloc = alloc.into();
        self.pools[alloc.buffer.binding].free(alloc);
    }

    crate fn boxed<T>(&mut self, binding: BufferBinding, val: T) ->
        BufferBox<T>
    {
        let size = std::mem::size_of::<T>();
        let alloc = self.alloc(binding, MemoryMapping::Mapped, size as _);
        BufferBox::from_val(alloc, val)
    }

    crate fn box_iter<T>(
        &mut self,
        binding: BufferBinding,
        iter: impl Iterator<Item = T> + ExactSizeIterator,
    ) -> BufferBox<[T]> {
        let size = std::mem::size_of::<T>() * iter.len();
        let alloc = self.alloc(binding, MemoryMapping::Mapped, size as _);
        BufferBox::from_iter(alloc, iter)
    }

    crate fn box_slice<T: Copy>(
        &mut self,
        binding: BufferBinding,
        src: &[T],
    ) -> BufferBox<[T]> {
        let size = std::mem::size_of::<T>() * src.len();
        let alloc = self.alloc(binding, MemoryMapping::Mapped, size as _);
        BufferBox::copy_from_slice(alloc, src)
    }
}

impl BufferHeapEntry {
    fn new(
        device: &Arc<Device>,
        binding: BufferBinding,
    ) -> Self {
        let mut mapped_pool = BufferPool::new(
            Arc::clone(&device),
            binding,
            MemoryMapping::Mapped,
        );

        // Pre-allocate a chunk of memory to infer if we're on UMA.
        unsafe { mapped_pool.add_chunk(1) };
        let flags = mapped_pool.chunks.first().unwrap().memory().flags();
        let device_local = vk::MemoryPropertyFlags::DEVICE_LOCAL_BIT;
        let unmapped_pool = flags.contains(device_local)
            .then(|| BufferPool::new(
                Arc::clone(&device),
                binding,
                MemoryMapping::Unmapped,
            ));

        BufferHeapEntry {
            binding,
            mapped_pool,
            unmapped_pool,
        }
    }

    fn pool_mut(&mut self, mapping: MemoryMapping) -> &mut BufferPool {
        // FTR the outer branch should always be predicted correctly
        if let Some(ref mut pool) = self.unmapped_pool {
            if mapping == MemoryMapping::Unmapped {
                return pool;
            }
        }
        &mut self.mapped_pool
    }

    fn alloc(&mut self, mapping: MemoryMapping, size: vk::DeviceSize) ->
        BufferRange
    {
        self.pool_mut(mapping).alloc(size)
    }

    fn free(&mut self, alloc: impl Into<BufferRange>) {
        let alloc = alloc.into();
        let pool = self.pool_mut(alloc.buffer.mapping);
        pool.free(alloc);
    }
}

impl Drop for BufferPool {
    fn drop(&mut self) {
        for chunk in self.chunks.iter() {
            assert_eq!(Arc::strong_count(chunk), 1,
                "allocator destroyed while chunk in use: {:?}", chunk);
        }
    }
}

impl BufferPool {
    fn new(
        device: Arc<Device>,
        binding: BufferBinding,
        mapping: MemoryMapping,
    ) -> Self {
        Self {
            device,
            binding,
            mapping,
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

    fn chunk_size(&self) -> vk::DeviceSize {
        0x100_0000
    }

    fn chunks(&self) -> &[Arc<DeviceBuffer>] {
        &self.chunks
    }

    fn alignment(&self) -> vk::DeviceSize {
        use BufferBinding::*;
        let limits = &self.device.limits();
        match self.binding {
            Storage => limits.min_storage_buffer_offset_alignment,
            Uniform => limits.min_uniform_buffer_offset_alignment,
            StorageTexel | UniformTexel =>
                limits.min_texel_buffer_offset_alignment,
            Vertex | Index => 1,
        }
    }

    fn usage(&self) -> vk::BufferUsageFlags {
        self.binding.usage() | self.mapping.usage()
    }

    fn mapping(&self) -> MemoryMapping {
        self.mapping
    }

    fn mapped(&self) -> bool {
        self.mapping().into()
    }

    // TODO: Dedicated allocation
    unsafe fn add_chunk(&mut self, min_size: vk::DeviceSize) {
        let dt = &*self.device.table;

        let chunk = self.chunks.len() as u32;
        let size = align(self.chunk_size(), min_size);

        let create_info = vk::BufferCreateInfo {
            size,
            usage: self.usage(),
            ..Default::default()
        };
        let mut buffer = vk::null();
        dt.create_buffer(&create_info, ptr::null(), &mut buffer)
            .check().unwrap();

        let mut reqs = Default::default();
        dt.get_buffer_memory_requirements(buffer, &mut reqs);

        let flags = self.mapping.memory_property_flags();
        let types = reqs.memory_type_bits;
        let type_index = find_memory_type(&self.device, flags, types).unwrap();

        let memory = alloc_device_memory(&self.device, size, type_index);
        let mut mem = DeviceMemory {
            device: Arc::clone(&self.device),
            inner: memory,
            size,
            type_index,
            ptr: 0 as _,
            tiling: Tiling::Linear,
            chunk,
        };
        mem.init();

        let buffer = DeviceBuffer {
            memory: Arc::new(mem),
            inner: buffer,
            usage: self.usage(),
            binding: self.binding,
            mapping: self.mapping,
        };

        self.chunks.push(Arc::new(buffer));
        self.allocator.add_chunk(size);
    }

    fn alloc(&mut self, size: vk::DeviceSize) -> BufferRange {
        let alignment = self.alignment();
        let size = align(alignment, size);

        assert_ne!(size, 0);
        let limits = self.device.limits();
        match self.binding {
            BufferBinding::Uniform =>
                assert!(size < limits.max_uniform_buffer_range as _),
            BufferBinding::Storage =>
                assert!(size < limits.max_storage_buffer_range as _),
            _ => (),
        }

        let block = self.allocator.alloc(size, alignment)
            .or_else(|| {
                unsafe { self.add_chunk(size); }
                self.allocator.alloc(size, alignment)
            })
            .unwrap();
        let buffer = Arc::clone(&self.chunks[block.chunk as usize]);
        BufferRange {
            buffer,
            offset: block.offset(),
            size: block.size(),
        }
    }

    fn free(&mut self, alloc: BufferRange) {
        // Make sure the allocation came from this pool
        assert!(Arc::ptr_eq(
            &alloc.buffer,
            &self.chunks[alloc.chunk() as usize],
        ));
        self.allocator.free(to_block(&alloc));
    }

    /// Invalidates existing allocations.
    unsafe fn clear(&mut self) {
        for chunk in self.chunks.iter() {
            assert_eq!(Arc::strong_count(chunk), 1,
                "chunk cleared while in use: {:?}", chunk);
        }
        self.allocator.clear()
    }
}

impl BufferBinding {
    fn usage(self) -> vk::BufferUsageFlags {
        use vk::BufferUsageFlags as Flags;
        use BufferBinding::*;
        match self {
            Storage => Flags::STORAGE_BUFFER_BIT,
            Uniform => Flags::UNIFORM_BUFFER_BIT,
            StorageTexel => Flags::STORAGE_TEXEL_BUFFER_BIT,
            UniformTexel => Flags::UNIFORM_TEXEL_BUFFER_BIT,
            Vertex => Flags::VERTEX_BUFFER_BIT,
            Index => Flags::INDEX_BUFFER_BIT,
        }
    }
}

#[cfg(test)]
mod tests {
    use vk::traits::*;
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        use BufferBinding::*;

        let device = Arc::clone(&vars.swapchain.device);
        let mut heap = BufferHeap::new(device);

        let x = heap.boxed(Uniform, [0.0f32, 0.5, 0.5, 1.0]);
        assert_eq!(x[1], 0.5);
        heap.free(x);
    }

    unsafe fn oversize_test(vars: testing::TestVars) {
        use BufferBinding::*;

        let device = Arc::clone(&vars.swapchain.device);
        let mut heap = BufferHeap::new(Arc::clone(&device));

        let x = heap.alloc(
            Uniform,
            MemoryMapping::Mapped,
            (2 * device.limits().max_uniform_buffer_range) as _
        );
        heap.free(x);
    }

    unit::declare_tests![
        smoke_test,
        (#[should_err] oversize_test),
    ];
}

unit::collect_tests![tests];
