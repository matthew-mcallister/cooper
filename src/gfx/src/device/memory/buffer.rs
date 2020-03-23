use std::ptr::{self, NonNull};
use std::sync::{Arc, Weak};

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
    // Lifetime of the contents.
    lifetime: Lifetime,
    heap: Weak<BufferHeap>,
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
unsafe impl<T: ?Sized> Send for BufferBox<T> {}
unsafe impl<T: ?Sized> Sync for BufferBox<T> {}

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
        unsafe { try_opt! { Weak::upgrade(&self.buffer.heap)?.free(self) }; }
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

    crate fn range(&self) -> BufferRange<'_> {
        self.alloc.range()
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
    // TODO: Mutex individual pools instead of the whole heap.
    inner: Mutex<BufferHeapInner>,
}

#[derive(Debug)]
crate struct BufferHeapInner {
    device: Arc<Device>,
    static_pools: EnumMap<BufferBinding, BufferHeapEntry<FreeListAllocator>>,
    frame_pools: EnumMap<BufferBinding, BufferHeapEntry<LinearAllocator>>,
}

#[derive(Debug)]
struct BufferHeapEntry<A: Allocator> {
    binding: BufferBinding,
    // Memory mapped pool and the only pool on UMA.
    mapped_pool: BufferPool<A>,
    // Unmapped, non-host-visible pool on discrete systems.
    unmapped_pool: Option<BufferPool<A>>,
}

#[derive(Debug)]
struct BufferPool<A: Allocator> {
    device: Arc<Device>,
    heap: Weak<BufferHeap>,
    binding: BufferBinding,
    lifetime: Lifetime,
    mapping: MemoryMapping,
    allocator: A,
    chunks: Vec<Arc<DeviceBuffer>>,
}

impl BufferHeap {
    crate fn new(device: Arc<Device>) -> Arc<Self> {
        macro_rules! entry {
            ($dev:expr, $lt:expr) => {
                (|binding| BufferHeapEntry::new($dev, binding, $lt)).into()
            }
        }
        let heap = Arc::new(Self {
            inner: Mutex::new(BufferHeapInner {
                static_pools: entry!(&device, Lifetime::Static),
                frame_pools: entry!(&device, Lifetime::Frame),
                device,
            }),
        });
        heap.assign_backpointers();
        heap
    }

    fn assign_backpointers(self: &Arc<Self>) {
        // Assign weak back-pointer
        let mut inner = self.inner.lock();
        for entry in inner.static_pools.values_mut() {
            entry.mapped_pool.heap = Arc::downgrade(self);
            if let Some(ref mut pool) = entry.unmapped_pool {
                pool.heap = Arc::downgrade(self);
            }
        }
    }

    crate fn alloc(
        self: &Arc<Self>,
        binding: BufferBinding,
        lifetime: Lifetime,
        mapping: MemoryMapping,
        size: vk::DeviceSize,
    ) -> BufferAlloc {
        trace!("allocating buffer memory: size: {}, {:?}, {:?}, {:?}",
            size, mapping, lifetime, binding);
        match lifetime {
            Lifetime::Static => self.inner.lock()
                .static_pools[binding]
                .alloc(mapping, size),
            Lifetime::Frame => self.inner.lock()
                .frame_pools[binding]
                .alloc(mapping, size),
        }
    }

    unsafe fn free(&self, alloc: &BufferAlloc) {
        let buffer = &alloc.buffer;
        if buffer.lifetime != Lifetime::Static { return; }
        trace!("freeing buffer memory: size: {}, {:?}, {:?}, {:?}",
            alloc.size, buffer.mapping, buffer.lifetime, buffer.binding);
        self.inner.lock().static_pools[alloc.buffer.binding].free(alloc);
    }

    crate fn boxed<T>(
        self: &Arc<Self>,
        binding: BufferBinding,
        lifetime: Lifetime,
        val: T,
    ) -> BufferBox<T> {
        let size = std::mem::size_of::<T>();
        let alloc = self.alloc(
            binding, lifetime, MemoryMapping::Mapped, size as _);
        BufferBox::from_val(alloc, val)
    }

    crate fn box_iter<T>(
        self: &Arc<Self>,
        binding: BufferBinding,
        lifetime: Lifetime,
        iter: impl Iterator<Item = T> + ExactSizeIterator,
    ) -> BufferBox<[T]> {
        let size = std::mem::size_of::<T>() * iter.len();
        let alloc = self.alloc(
            binding, lifetime, MemoryMapping::Mapped, size as _);
        BufferBox::from_iter(alloc, iter)
    }

    crate fn box_slice<T: Copy>(
        self: &Arc<Self>,
        binding: BufferBinding,
        lifetime: Lifetime,
        src: &[T],
    ) -> BufferBox<[T]> {
        let size = std::mem::size_of::<T>() * src.len();
        let alloc = self.alloc(
            binding, lifetime, MemoryMapping::Mapped, size as _);
        BufferBox::copy_from_slice(alloc, src)
    }

    /// Invalidates frame-scope allocations.
    crate unsafe fn clear_frame(&self) {
        for pool in self.inner.lock().frame_pools.values_mut() {
            pool.clear();
        }
    }
}

impl<A: Allocator> BufferHeapEntry<A> {
    fn new(
        device: &Arc<Device>,
        binding: BufferBinding,
        lifetime: Lifetime,
    ) -> Self {
        let mut mapped_pool = BufferPool::new(
            Arc::clone(&device),
            binding,
            lifetime,
            MemoryMapping::Mapped,
        );

        // Pre-allocate a chunk of memory to infer if we're on UMA.
        // TODO: Free memory afterward?
        unsafe { mapped_pool.add_chunk(1) };
        let flags = mapped_pool.chunks.first().unwrap().memory().flags();
        let device_local = vk::MemoryPropertyFlags::DEVICE_LOCAL_BIT;
        let unmapped_pool = flags.contains(device_local)
            .then(|| BufferPool::new(
                Arc::clone(&device),
                binding,
                lifetime,
                MemoryMapping::Unmapped,
            ));

        BufferHeapEntry {
            binding,
            mapped_pool,
            unmapped_pool,
        }
    }

    fn pool_mut(&mut self, mapping: MemoryMapping) -> &mut BufferPool<A> {
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

    unsafe fn clear(&mut self) {
        self.mapped_pool.clear();
        if let Some(pool) = self.unmapped_pool.as_mut() {
            pool.clear();
        }
    }
}

impl<A: Allocator> Drop for BufferPool<A> {
    fn drop(&mut self) {
        if std::thread::panicking() { return; }
        for chunk in self.chunks.iter() {
            assert_eq!(Arc::strong_count(chunk), 1,
                "allocator destroyed while chunk in use: {:?}", chunk);
        }
    }
}

impl<A: Allocator> BufferPool<A> {
    fn new(
        device: Arc<Device>,
        binding: BufferBinding,
        lifetime: Lifetime,
        mapping: MemoryMapping,
    ) -> Self {
        Self {
            device,
            heap: Weak::new(),
            binding,
            lifetime,
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
            lifetime: self.lifetime,
            binding: self.binding,
            mapping: self.mapping,
            heap: Weak::clone(&self.heap),
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
        use Lifetime::*;
        use MemoryMapping::*;

        let device = Arc::clone(&vars.swapchain.device);
        let heap = Arc::new(BufferHeap::new(device));

        let x = heap.boxed(Uniform, Static, [0.0f32, 0.5, 0.5, 1.0]);
        assert_eq!(x[1], 0.5);

        heap.alloc(Uniform, Frame, Unmapped, 256);
        heap.clear_frame();
        // TODO: Query used memory
    }

    unsafe fn oversize_test(vars: testing::TestVars) {
        use BufferBinding::*;
        use Lifetime::*;

        let device = Arc::clone(&vars.swapchain.device);
        let heap = Arc::new(BufferHeap::new(Arc::clone(&device)));

        let _ = heap.alloc(
            Uniform,
            Static,
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
