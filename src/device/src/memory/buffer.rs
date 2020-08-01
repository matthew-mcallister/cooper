use std::mem::MaybeUninit;
use std::ptr::{self, NonNull};
use std::sync::{Arc, Weak};

use enum_map::{Enum, EnumMap};
use parking_lot::Mutex;
use prelude::*;

use super::*;

#[derive(Clone, Debug)]
pub struct DeviceBuffer {
    memory: Arc<DeviceMemory>,
    inner: vk::Buffer,
    usage: vk::BufferUsageFlags,
    binding: Option<BufferBinding>,
    heap: Weak<BufferHeap>,
    name: Option<String>,
}

#[derive(Clone, Copy, Debug, Enum, Eq, Hash, PartialEq)]
pub enum BufferBinding {
    Storage,
    Uniform,
    StorageTexel,
    UniformTexel,
    Vertex,
    Index,
}

// A slice of a VkBuffer.
#[derive(Clone, Copy, Debug)]
pub struct BufferRange<'buf> {
    pub buffer: &'buf DeviceBuffer,
    pub offset: vk::DeviceSize,
    pub size: vk::DeviceSize,
}

/// An owned suballocation of a VkBuffer object.
#[derive(Debug)]
pub struct BufferAlloc {
    buffer: Arc<DeviceBuffer>,
    offset: vk::DeviceSize,
    // N.B. the allocator might return more memory than requested.
    size: vk::DeviceSize,
}

/// Warning: This type does *not* call `T::drop`. Unfortunately Rust
/// does not allow us to express this as a type constraint yet.
#[derive(Debug)]
pub struct BufferBox<T: ?Sized> {
    alloc: BufferAlloc,
    ptr: NonNull<T>,
}

impl Drop for DeviceBuffer {
    fn drop(&mut self) {
        let dt = self.device().table();
        unsafe { dt.destroy_buffer(self.inner, ptr::null()); }
    }
}

unsafe fn create_buffer(
    device: Arc<Device>,
    size: vk::DeviceSize,
    usage: vk::BufferUsageFlags,
    mapping: MemoryMapping,
    lifetime: Lifetime,
    chunk: Option<u32>,
) -> DeviceBuffer {
    trace!("create_buffer({:?}, {:?}, {:?}, {:?}, {:?}, {:?})",
        device, size, usage, mapping, lifetime, chunk);

    let dt = device.table();

    let create_info = vk::BufferCreateInfo {
        size,
        usage,
        ..Default::default()
    };
    let mut buffer = vk::null();
    dt.create_buffer(&create_info, ptr::null(), &mut buffer)
        .check().unwrap();

    let (reqs, dedicated_reqs) = get_buffer_memory_reqs(&device, buffer);
    let content = (dedicated_reqs.prefers_dedicated_allocation == vk::TRUE)
        .then_some(DedicatedAllocContent::Buffer(buffer));
    let mut memory = alloc_resource_memory(
        device,
        mapping,
        &reqs,
        content,
        Tiling::Linear,
    );
    memory.lifetime = lifetime;
    if let Some(chunk) = chunk {
        memory.chunk = chunk;
    }

    let mut buffer = DeviceBuffer {
        memory: Arc::new(memory),
        inner: buffer,
        usage,
        binding: None,
        heap: Weak::new(),
        name: None,
    };
    buffer.bind();

    buffer
}

impl DeviceBuffer {
    pub(super) unsafe fn new(
        device: Arc<Device>,
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        mapping: MemoryMapping,
        lifetime: Lifetime,
        chunk: Option<u32>,
    ) -> Self {
        create_buffer(device, size, usage, mapping, lifetime, chunk)
    }

    pub fn device(&self) -> &Arc<Device> {
        &self.memory.device
    }

    pub fn memory(&self) -> &Arc<DeviceMemory> {
        &self.memory
    }

    pub fn inner(&self) -> vk::Buffer {
        self.inner
    }

    pub fn size(&self) -> vk::DeviceSize {
        self.memory.size()
    }

    pub fn lifetime(&self) -> Lifetime {
        self.memory.lifetime()
    }

    pub fn mapped(&self) -> bool {
        self.memory.mapped()
    }

    pub fn binding(&self) -> Option<BufferBinding> {
        self.binding
    }

    pub fn usage(&self) -> vk::BufferUsageFlags {
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

    pub fn set_name(&mut self, name: impl Into<String>) {
        let name: String = name.into();
        self.name = Some(name.clone());
        unsafe { self.device().set_name(self.inner(), name); }
    }
}

impl Named for DeviceBuffer {
    fn name(&self) -> Option<&str> {
        Some(&self.name.as_ref()?)
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
        unsafe { tryopt! { Weak::upgrade(&self.buffer.heap)?.free(self) }; }
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
    pub fn raw(&self) -> vk::Buffer {
        self.buffer.inner
    }

    pub fn descriptor_info(&self) -> vk::DescriptorBufferInfo {
        vk::DescriptorBufferInfo {
            buffer: self.buffer.inner,
            offset: self.offset,
            range: self.size,
        }
    }
}

impl BufferAlloc {
    pub fn buffer(&self) -> &Arc<DeviceBuffer> {
        &self.buffer
    }

    pub fn raw(&self) -> vk::Buffer {
        self.buffer.inner
    }

    fn chunk(&self) -> u32 {
        self.buffer.memory.chunk
    }

    pub fn range(&self) -> BufferRange<'_> {
        BufferRange {
            buffer: &self.buffer,
            offset: self.offset,
            size: self.size,
        }
    }

    /// Destroys `self` without deallocating memory.
    fn leak(self) {
        let this = MaybeUninit::new(self);
        unsafe { std::ptr::read(&this.get_ref().buffer as *const Arc<_>); }
    }
}

unsafe impl<T: ?Sized> Send for BufferBox<T> {}
unsafe impl<T: ?Sized> Sync for BufferBox<T> {}

impl<T: ?Sized> AsRef<BufferAlloc> for BufferBox<T> {
    fn as_ref(&self) -> &BufferAlloc {
        &self.alloc
    }
}

impl<T: ?Sized> BufferBox<T> {
    unsafe fn new(alloc: BufferAlloc, ptr: *mut T) -> Self {
        BufferBox { alloc, ptr: NonNull::new(ptr).unwrap() }
    }

    pub fn alloc(this: &Self) -> &BufferAlloc {
        &this.alloc
    }

    pub fn range(this: &Self) -> BufferRange<'_> {
        this.alloc.range()
    }

    pub fn into_inner(this: Self) -> BufferAlloc {
        this.alloc
    }

    /// Unlike `Box::leak`, the pointer returned by this method deosn't
    /// have `'static` lifetime---it may dangle. Hence, it is not safe
    /// to dereference.
    pub fn leak(this: Self) -> NonNull<T> {
        this.alloc.leak();
        this.ptr
    }
}

impl<T> BufferBox<T> {
    pub fn from_val(mut alloc: BufferAlloc, val: T) -> Self {
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
    pub fn copy_from_slice(mut alloc: BufferAlloc, src: &[T]) -> Self {
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
pub struct BufferHeap {
    // TODO: Mutex individual pools instead of the whole heap.
    inner: Mutex<BufferHeapInner>,
}

#[derive(Debug)]
pub struct BufferHeapInner {
    device: Arc<Device>,
    static_pools: EnumMap<BufferBinding, BufferHeapEntry<FreeListAllocator>>,
    frame_pools: EnumMap<BufferBinding, BufferHeapEntry<LinearAllocator>>,
}

#[derive(Debug)]
struct BufferHeapEntry<A: Allocator> {
    binding: BufferBinding,
    // Memory mapped pool. The only pool on UMA.
    mapped_pool: BufferPool<A>,
    // Unmapped, device-local, non-host-visible pool. Only present on
    // discrete systems.
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
    pub fn new(device: Arc<Device>) -> Arc<Self> {
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
        // Assign weak back-pointer to heap on each pool
        let mut inner = self.inner.lock();
        for entry in inner.static_pools.values_mut() {
            entry.mapped_pool.heap = Arc::downgrade(self);
            if let Some(ref mut pool) = entry.unmapped_pool {
                pool.heap = Arc::downgrade(self);
            }
        }
    }

    pub fn alloc(
        self: &Arc<Self>,
        binding: BufferBinding,
        lifetime: Lifetime,
        mapping: MemoryMapping,
        size: vk::DeviceSize,
    ) -> BufferAlloc {
        trace!("BufferHeap::alloc({:?}, {:?}, {:?}, {:?})",
            binding, lifetime, mapping, size);
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
        trace!("BufferHeap::free({:?})", alloc);
        let buffer = &alloc.buffer;
        if buffer.lifetime() != Lifetime::Static { return; }
        self.inner.lock().static_pools[alloc.buffer.binding.unwrap()]
            .free(alloc);
    }

    pub fn boxed<T>(
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

    pub fn box_iter<T>(
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

    pub fn box_slice<T: Copy>(
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

    pub fn box_uninit<T: Copy>(
        self: &Arc<Self>,
        binding: BufferBinding,
        lifetime: Lifetime,
        len: usize,
    ) -> BufferBox<[MaybeUninit<T>]> {
        let size = std::mem::size_of::<T>() * len;
        let mut alloc = self.alloc(
            binding, lifetime, MemoryMapping::Mapped, size as _);
        let ptr = alloc.as_mut_slice::<T>(len) as *mut _;
        unsafe { BufferBox::new(alloc, ptr) }
    }

    /// Invalidates frame-scope allocations.
    pub unsafe fn clear_frame(&self) {
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
        let unmapped_pool = (!flags.contains(device_local))
            .then(|| BufferPool::new(
                Arc::clone(&device),
                binding,
                lifetime,
                MemoryMapping::DeviceLocal,
            ));

        BufferHeapEntry {
            binding,
            mapped_pool,
            unmapped_pool,
        }
    }

    fn pick_pool(&mut self, mapping: MemoryMapping) -> &mut BufferPool<A> {
        if let Some(ref mut pool) = self.unmapped_pool {
            if mapping == MemoryMapping::DeviceLocal {
                return pool;
            }
        }
        &mut self.mapped_pool
    }

    fn alloc(&mut self, mapping: MemoryMapping, size: vk::DeviceSize) ->
        BufferAlloc
    {
        self.pick_pool(mapping).alloc(size)
    }

    fn get_pool(&mut self, mapped: bool) -> &mut BufferPool<A> {
        if mapped {
            &mut self.mapped_pool
        } else {
            self.unmapped_pool.as_mut().unwrap()
        }
    }

    fn free(&mut self, alloc: &BufferAlloc) {
        self.get_pool(alloc.buffer().mapped()).free(alloc);
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
                concat!(
                    "allocator destroyed while chunk in use: {:?};\n",
                    "make sure all resources are destroyed before the\n",
                    "render loop is destroyed",
                ), fmt_named(&**chunk));
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

    #[allow(dead_code)]
    fn used(&self) -> vk::DeviceSize {
        self.allocator.used()
    }

    #[allow(dead_code)]
    fn reserved(&self) -> vk::DeviceSize {
        self.allocator.capacity()
    }

    fn chunk_size(&self) -> vk::DeviceSize {
        0x100_0000
    }

    #[allow(dead_code)]
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
        self.binding.usage()
            // TODO: It's probably not necessary to set *both* flags,
            // but it is convenient to and I don't know of any
            // implementations that even read these bits
            | vk::BufferUsageFlags::TRANSFER_SRC_BIT
            | vk::BufferUsageFlags::TRANSFER_DST_BIT
    }

    #[allow(dead_code)]
    fn mapping(&self) -> MemoryMapping {
        self.mapping
    }

    unsafe fn add_chunk(&mut self, min_size: vk::DeviceSize) {
        let chunk = self.chunks.len() as u32;
        let size = align(self.chunk_size(), min_size);
        let mut buffer = DeviceBuffer::new(
            Arc::clone(&self.device),
            size,
            self.usage(),
            self.mapping,
            self.lifetime,
            Some(chunk),
        );
        buffer.binding = Some(self.binding);
        buffer.heap = Weak::clone(&self.heap);

        buffer.set_name(format!(
            "{:?}|{:?}|{:?}[{}]",
            self.binding,
            self.lifetime,
            self.mapping,
            chunk,
        ));

        self.chunks.push(Arc::new(buffer));
        self.allocator.add_chunk(size);
    }

    fn alloc(&mut self, size: vk::DeviceSize) -> BufferAlloc {
        assert_ne!(size, 0);
        let alignment = self.alignment();

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
        let chunk = &self.chunks[alloc.chunk() as usize];
        assert!(
            Arc::ptr_eq(&alloc.buffer, chunk),
            "alloc: {:?},\nself.chunks[alloc.chunk]: {:?}",
            alloc, chunk,
        );
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

    unsafe fn create_buffer(vars: testing::TestVars) {
        DeviceBuffer::new(
            Arc::clone(vars.device()),
            8 * (2 << 20),
            vk::BufferUsageFlags::TRANSFER_SRC_BIT,
            MemoryMapping::Mapped,
            Lifetime::Static,
            None,
        );
    }

    unsafe fn heap_alloc(vars: testing::TestVars) {
        use BufferBinding::*;
        use Lifetime::*;
        use MemoryMapping::*;

        let device = Arc::clone(&vars.swapchain.device);
        let heap = Arc::new(BufferHeap::new(device));

        let x = heap.boxed(Uniform, Static, [0.0f32, 0.5, 0.5, 1.0]);
        assert_eq!(x[1], 0.5);

        heap.alloc(Uniform, Frame, DeviceLocal, 256);
        heap.clear_frame();
        // TODO: Query used memory
    }

    unsafe fn oversized_alloc(vars: testing::TestVars) {
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
        create_buffer,
        heap_alloc,
        (#[should_err] oversized_alloc),
    ];
}

unit::collect_tests![tests];
