use std::ptr::{self, NonNull};
use std::sync::Arc;

use enum_map::{Enum, EnumMap};
use parking_lot::Mutex;
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

#[derive(Clone, Copy, Debug, Enum, Eq, Hash, PartialEq)]
crate enum BufferBinding {
    Storage,
    Uniform,
    StorageTexel,
    UniformTexel,
    Vertex,
    Index,
}

#[derive(Debug)]
crate struct BufferBox<T: ?Sized> {
    alloc: BufferAlloc,
    ptr: NonNull<T>,
}

// A slice of a VkBuffer.
#[derive(Clone, Copy, Debug)]
crate struct BufferRange<'buf> {
    crate buffer: &'buf DeviceBuffer,
    crate offset: vk::DeviceSize,
    crate size: vk::DeviceSize,
}

/// An owned suballocation of a VkBuffer object.
#[derive(Debug)]
crate struct BufferAlloc {
    buffer: Arc<DeviceBuffer>,
    offset: vk::DeviceSize,
    // N.B. the allocator might return more memory than requested.
    size: vk::DeviceSize,
    // When `None`, the buffer has transient lifetime.
    heap: Option<Arc<BufferHeap>>,
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

    crate fn binding(&self) -> BufferBinding {
        self.binding
    }

    crate fn usage(&self) -> vk::BufferUsageFlags {
        self.usage
    }

    unsafe fn bind(&mut self) {
        let dt = &*self.device().table;
        assert_ne!(self.inner, vk::null());
        if let Some(content) = self.memory.dedicated_content {
            assert_eq!(DedicatedAllocContent::Buffer(self.inner), content);
        }
        dt.bind_buffer_memory(self.inner, self.memory.inner(), 0);
    }
}

impl<'a> MemoryRegion for BufferRange<'a> {
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

impl Drop for BufferAlloc {
    fn drop(&mut self) {
        if let Some(heap) = self.heap.take() {
            unsafe { heap.free(self); }
        }
    }
}

impl MemoryRegion for BufferAlloc {
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

impl<'a> BufferRange<'a> {
    crate fn raw(&self) -> vk::Buffer {
        self.buffer.inner
    }

    crate fn descriptor_info(&self) -> vk::DescriptorBufferInfo {
        vk::DescriptorBufferInfo {
            buffer: self.buffer.inner,
            offset: self.offset,
            range: self.size,
        }
    }
}

impl BufferAlloc {
    crate fn buffer(&self) -> &Arc<DeviceBuffer> {
        &self.buffer
    }

    crate fn raw(&self) -> vk::Buffer {
        self.buffer.inner
    }

    fn chunk(&self) -> u32 {
        self.buffer.memory.chunk
    }

    crate fn range(&self) -> BufferRange<'_> {
        BufferRange {
            buffer: &self.buffer,
            offset: self.offset,
            size: self.size,
        }
    }
}

impl<T: ?Sized> Drop for BufferBox<T> {
    fn drop(&mut self) {
        unsafe { std::ptr::drop_in_place(self.ptr.as_ptr()); }
    }
}

impl<T: ?Sized> AsRef<BufferAlloc> for BufferBox<T> {
    fn as_ref(&self) -> &BufferAlloc {
        &self.alloc
    }
}

impl<T: ?Sized> From<BufferBox<T>> for BufferAlloc {
    fn from(data: BufferBox<T>) -> Self {
        data.into_inner()
    }
}

impl<T: ?Sized> BufferBox<T> {
    unsafe fn new(alloc: BufferAlloc, ptr: *mut T) -> Self {
        BufferBox { alloc, ptr: NonNull::new_unchecked(ptr) }
    }

    crate fn alloc(&self) -> &BufferAlloc {
        &self.alloc
    }

    crate fn into_inner(self) -> BufferAlloc {
        unsafe {
            std::ptr::drop_in_place(self.ptr.as_ptr());
            let alloc = ptr::read(&self.alloc);
            std::mem::forget(self);
            alloc
        }
    }
}

impl<T> BufferBox<T> {
    crate fn from_val(mut alloc: BufferAlloc, val: T) -> Self {
        let ptr = alloc.as_mut::<T>().write(val) as _;
        unsafe { BufferBox::new(alloc, ptr) }
    }
}

impl<T> BufferBox<[T]> {
    fn from_iter(
        mut alloc: BufferAlloc,
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
    crate fn copy_from_slice(mut alloc: BufferAlloc, src: &[T]) -> Self {
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
    inner: Mutex<BufferHeapInner>,
}

// TODO: Frame-local allocations
#[derive(Debug)]
crate struct BufferHeapInner {
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

impl BufferHeap {
    crate fn new(device: Arc<Device>) -> Self {
        let pools = (|binding| BufferHeapEntry::new(&device, binding)).into();
        Self {
            inner: Mutex::new(BufferHeapInner {
                device,
                pools,
            }),
        }
    }

    crate fn alloc(
        self: &Arc<Self>,
        binding: BufferBinding,
        mapping: MemoryMapping,
        size: vk::DeviceSize,
    ) -> BufferAlloc {
        trace!("allocating buffer memory: size: {}, {:?}, {:?}",
            size, mapping, binding);
        let mut alloc = self.inner.lock().pools[binding].alloc(mapping, size);
        alloc.heap = Some(Arc::clone(self));
        alloc
    }

    unsafe fn free(&self, alloc: &BufferAlloc) {
        trace!("freeing buffer memory: size: {}, {:?}, {:?}",
            alloc.size, alloc.buffer.mapping, alloc.buffer.binding);
        self.inner.lock().pools[alloc.buffer.binding].free(alloc);
    }

    crate fn boxed<T>(self: &Arc<Self>, binding: BufferBinding, val: T) ->
        BufferBox<T>
    {
        let size = std::mem::size_of::<T>();
        let alloc = self.alloc(binding, MemoryMapping::Mapped, size as _);
        BufferBox::from_val(alloc, val)
    }

    crate fn box_iter<T>(
        self: &Arc<Self>,
        binding: BufferBinding,
        iter: impl Iterator<Item = T> + ExactSizeIterator,
    ) -> BufferBox<[T]> {
        let size = std::mem::size_of::<T>() * iter.len();
        let alloc = self.alloc(binding, MemoryMapping::Mapped, size as _);
        BufferBox::from_iter(alloc, iter)
    }

    crate fn box_slice<T: Copy>(
        self: &Arc<Self>,
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
        BufferAlloc
    {
        self.pool_mut(mapping).alloc(size)
    }

    fn free(&mut self, alloc: &BufferAlloc) {
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

        let (reqs, dedicated_reqs) =
            get_buffer_memory_reqs(&self.device, buffer);
        let content = (dedicated_reqs.prefers_dedicated_allocation == vk::TRUE)
            .then_some(DedicatedAllocContent::Buffer(buffer));
        let mut memory = alloc_resource_memory(
            Arc::clone(&self.device),
            self.mapping,
            &reqs,
            content,
            Tiling::Linear,
        );
        memory.chunk = chunk;

        let mut buffer = DeviceBuffer {
            memory: Arc::new(memory),
            inner: buffer,
            usage: self.usage(),
            binding: self.binding,
            mapping: self.mapping,
        };
        buffer.bind();

        self.chunks.push(Arc::new(buffer));
        self.allocator.add_chunk(size);
    }

    fn alloc(&mut self, size: vk::DeviceSize) -> BufferAlloc {
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
        BufferAlloc {
            buffer,
            offset: block.offset(),
            size: block.size(),
            heap: None,
        }
    }

    fn free(&mut self, alloc: &BufferAlloc) {
        // Make sure the allocation came from this pool
        assert!(Arc::ptr_eq(
            &alloc.buffer,
            &self.chunks[alloc.chunk() as usize],
        ));
        self.allocator.free(to_block(alloc));
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
        let heap = Arc::new(BufferHeap::new(device));

        let x = heap.boxed(Uniform, [0.0f32, 0.5, 0.5, 1.0]);
        assert_eq!(x[1], 0.5);
    }

    unsafe fn oversize_test(vars: testing::TestVars) {
        use BufferBinding::*;

        let device = Arc::clone(&vars.swapchain.device);
        let heap = Arc::new(BufferHeap::new(Arc::clone(&device)));

        let _ = heap.alloc(
            Uniform,
            MemoryMapping::Mapped,
            (2 * device.limits().max_uniform_buffer_range) as _
        );
    }

    unit::declare_tests![
        smoke_test,
        (#[should_err] oversize_test),
    ];
}

unit::collect_tests![tests];
