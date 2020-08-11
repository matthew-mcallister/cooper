use std::mem::MaybeUninit;
use std::ptr::{self, NonNull};
use std::sync::{Arc, Weak};

use enum_map::Enum;

use crate::util::as_uninit_slice;
use super::*;

#[derive(Clone, Debug)]
pub struct DeviceBuffer {
    pub(super) memory: Arc<DeviceMemory>,
    pub(super) inner: vk::Buffer,
    pub(super) usage: vk::BufferUsageFlags,
    pub(super) binding: Option<BufferBinding>,
    pub(super) heap: Weak<BufferHeap>,
    pub(super) name: Option<String>,
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
    pub(super) buffer: Arc<DeviceBuffer>,
    pub(super) offset: vk::DeviceSize,
    // N.B. the allocator might return more memory than requested.
    pub(super) size: vk::DeviceSize,
}

/// # Caveats
///
/// This type will *not* ever call `T::drop`, so it is recommended you
/// avoid using `T` with a non-trivial destructor.
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

    #[inline]
    pub fn device(&self) -> &Arc<Device> {
        &self.memory.device
    }

    #[inline]
    pub fn memory(&self) -> &Arc<DeviceMemory> {
        &self.memory
    }

    #[inline]
    pub fn inner(&self) -> vk::Buffer {
        self.inner
    }

    #[inline]
    pub fn size(&self) -> vk::DeviceSize {
        self.memory.size()
    }

    #[inline]
    pub fn lifetime(&self) -> Lifetime {
        self.memory.lifetime()
    }

    #[inline]
    pub fn mapped(&self) -> bool {
        self.memory.mapped()
    }

    #[inline]
    pub fn binding(&self) -> Option<BufferBinding> {
        self.binding
    }

    #[inline]
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
        if self.buffer.lifetime() == Lifetime::Static {
            unsafe { Weak::upgrade(&self.buffer.heap).unwrap().free(self); }
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
    #[inline]
    pub fn raw(&self) -> vk::Buffer {
        self.buffer.inner
    }

    #[inline]
    pub fn descriptor_info(&self) -> vk::DescriptorBufferInfo {
        vk::DescriptorBufferInfo {
            buffer: self.buffer.inner,
            offset: self.offset,
            range: self.size,
        }
    }
}

impl BufferAlloc {
    #[inline]
    pub fn buffer(&self) -> &Arc<DeviceBuffer> {
        &self.buffer
    }

    #[inline]
    pub fn raw(&self) -> vk::Buffer {
        self.buffer.inner
    }

    pub(super) fn chunk(&self) -> u32 {
        self.buffer.memory.chunk
    }

    #[inline]
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
        // Decrement the reference count
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

impl<T> BufferBox<MaybeUninit<T>> {
    pub fn new(alloc: BufferAlloc) -> Self {
        Self { ptr: unsafe { alloc.as_ptr().unwrap() }, alloc }
    }

    pub unsafe fn assume_init(self) -> BufferBox<T> {
        BufferBox { ptr: self.ptr.cast(), alloc: self.alloc }
    }
}

impl<T> BufferBox<[MaybeUninit<T>]> {
    pub fn new_slice(alloc: BufferAlloc, len: usize) -> Self {
        Self { ptr: unsafe { alloc.as_slice_ptr(len).unwrap() }, alloc }
    }

    pub unsafe fn assume_init_slice(self) -> BufferBox<[T]> {
        BufferBox {
            ptr: std::mem::transmute(self.ptr),
            alloc: self.alloc
        }
    }
}

impl<T: ?Sized> BufferBox<T> {
    #[inline]
    pub fn alloc(this: &Self) -> &BufferAlloc {
        &this.alloc
    }

    #[inline]
    pub fn range(this: &Self) -> BufferRange<'_> {
        this.alloc.range()
    }

    #[inline]
    pub fn into_inner(this: Self) -> BufferAlloc {
        this.alloc
    }

    /// Unlike `Box::leak`, the pointer returned by this method deosn't
    /// have `'static` lifetime---it may dangle. Hence, it is not safe
    /// to dereference.
    #[inline]
    pub fn leak(this: Self) -> NonNull<T> {
        this.alloc.leak();
        this.ptr
    }
}

impl<T> BufferBox<T> {
    #[inline]
    pub fn from_val(alloc: BufferAlloc, val: T) -> Self {
        let mut buf = BufferBox::new(alloc);
        buf.write(val);
        unsafe { buf.assume_init() }
    }
}

impl<T> BufferBox<[T]> {
    pub fn from_iter(
        alloc: BufferAlloc,
        iter: impl Iterator<Item = T> + ExactSizeIterator,
    ) -> Self {
        let len = alloc.size() as usize / std::mem::size_of::<T>();
        let mut buf = BufferBox::new_slice(alloc, len);
        for (dst, src) in buf.iter_mut().zip(iter) {
            dst.write(src);
        }
        unsafe { buf.assume_init_slice() }
    }
}

impl<T: Copy> BufferBox<[T]> {
    #[inline]
    pub fn copy_from_slice(alloc: BufferAlloc, src: &[T]) -> Self {
        let mut buf = BufferBox::new_slice(alloc, src.len());
        buf.copy_from_slice(as_uninit_slice(src));
        unsafe { buf.assume_init_slice() }
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
