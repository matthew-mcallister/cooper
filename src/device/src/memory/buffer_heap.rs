use std::mem::MaybeUninit;
use std::sync::{Arc, Weak};

use enum_map::EnumMap;
use parking_lot::Mutex;
use prelude::*;

use super::*;

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
            };
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

    // Assign weak back-pointer to self on each static pool/buffer
    // (this sucks on multiple layers but at least it's safe).
    fn assign_backpointers(self: &Arc<Self>) {
        impl BufferHeapEntry<FreeListAllocator> {
            fn assign_backpointers(&mut self, ptr: &Arc<BufferHeap>) {
                self.mapped_pool.assign_backpointers(ptr);
                tryopt! {
                    self.unmapped_pool.as_mut()?.assign_backpointers(ptr);
                };
            }
        }

        impl BufferPool<FreeListAllocator> {
            fn assign_backpointers(&mut self, ptr: &Arc<BufferHeap>) {
                self.heap = Arc::downgrade(ptr);
                for chunk in self.chunks.iter_mut() {
                    let buffer = Arc::get_mut(chunk).unwrap();
                    buffer.heap = Arc::downgrade(ptr);
                }
            }
        }

        let mut inner = self.inner.lock();
        for entry in inner.static_pools.values_mut() {
            entry.assign_backpointers(self);
        }
    }

    pub fn alloc(
        self: &Arc<Self>,
        binding: BufferBinding,
        lifetime: Lifetime,
        mapping: MemoryMapping,
        size: vk::DeviceSize,
    ) -> BufferAlloc {
        trace!(
            "BufferHeap::alloc({:?}, {:?}, {:?}, {:?})",
            binding,
            lifetime,
            mapping,
            size
        );
        match lifetime {
            Lifetime::Static => self.inner.lock().static_pools[binding].alloc(mapping, size),
            Lifetime::Frame => self.inner.lock().frame_pools[binding].alloc(mapping, size),
        }
    }

    pub(super) unsafe fn free(&self, alloc: &BufferAlloc) {
        trace!("BufferHeap::free({:?})", alloc);
        let buffer = &alloc.buffer;
        if buffer.lifetime() != Lifetime::Static {
            return;
        }
        self.inner.lock().static_pools[alloc.buffer.binding.unwrap()].free(alloc);
    }

    #[inline]
    pub fn boxed<T>(
        self: &Arc<Self>,
        binding: BufferBinding,
        lifetime: Lifetime,
        val: T,
    ) -> BufferBox<T> {
        let size = std::mem::size_of::<T>();
        let alloc = self.alloc(binding, lifetime, MemoryMapping::Mapped, size as _);
        BufferBox::from_val(alloc, val)
    }

    #[inline]
    pub fn box_iter<T>(
        self: &Arc<Self>,
        binding: BufferBinding,
        lifetime: Lifetime,
        iter: impl Iterator<Item = T> + ExactSizeIterator,
    ) -> BufferBox<[T]> {
        let size = std::mem::size_of::<T>() * iter.len();
        let alloc = self.alloc(binding, lifetime, MemoryMapping::Mapped, size as _);
        BufferBox::from_iter(alloc, iter)
    }

    #[inline]
    pub fn box_slice<T: Copy>(
        self: &Arc<Self>,
        binding: BufferBinding,
        lifetime: Lifetime,
        src: &[T],
    ) -> BufferBox<[T]> {
        let size = std::mem::size_of::<T>() * src.len();
        let alloc = self.alloc(binding, lifetime, MemoryMapping::Mapped, size as _);
        BufferBox::copy_from_slice(alloc, src)
    }

    #[inline]
    pub fn box_uninit<T: Copy>(
        self: &Arc<Self>,
        binding: BufferBinding,
        lifetime: Lifetime,
        len: usize,
    ) -> BufferBox<[MaybeUninit<T>]> {
        let size = std::mem::size_of::<T>() * len;
        let alloc = self.alloc(binding, lifetime, MemoryMapping::Mapped, size as _);
        BufferBox::new_slice(alloc, len)
    }

    /// Invalidates frame-scope allocations.
    pub unsafe fn clear_frame(&self) {
        for pool in self.inner.lock().frame_pools.values_mut() {
            pool.clear();
        }
    }
}

impl<A: Allocator> BufferHeapEntry<A> {
    fn new(device: &Arc<Device>, binding: BufferBinding, lifetime: Lifetime) -> Self {
        let mut mapped_pool = BufferPool::new(
            Arc::clone(&device),
            binding,
            lifetime,
            MemoryMapping::Mapped,
        );

        // We must call GetBufferMemoryRequirements to find out which
        // memory type index we will use. Thus, we pre-allocate a chunk
        // of memory to infer if we're on UMA.
        // TODO: Free memory afterward?
        mapped_pool.add_chunk(1);
        let flags = mapped_pool.chunks.first().unwrap().memory().flags();
        let device_local = vk::MemoryPropertyFlags::DEVICE_LOCAL_BIT;
        let unmapped_pool = (!flags.contains(device_local)).then(|| {
            BufferPool::new(
                Arc::clone(&device),
                binding,
                lifetime,
                MemoryMapping::DeviceLocal,
            )
        });

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

    fn alloc(&mut self, mapping: MemoryMapping, size: vk::DeviceSize) -> BufferAlloc {
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
        if std::thread::panicking() {
            return;
        }
        for chunk in self.chunks.iter() {
            assert_eq!(
                Arc::strong_count(chunk),
                1,
                concat!(
                    "allocator destroyed while chunk in use: {:?};\n",
                    "make sure all resources are destroyed before the\n",
                    "render loop is destroyed",
                ),
                fmt_named(&**chunk)
            );
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
            StorageTexel | UniformTexel => limits.min_texel_buffer_offset_alignment,
            Vertex | Index => 1,
        }
    }

    fn usage(&self) -> BufferUsage {
        self.binding.usage()
            // TODO: It's probably not necessary to set *both* flags,
            // but it is convenient to and I don't know of any
            // implementations that even read these bits
            | BufferUsage::TRANSFER_SRC
            | BufferUsage::TRANSFER_DST
    }

    #[allow(dead_code)]
    fn mapping(&self) -> MemoryMapping {
        self.mapping
    }

    fn add_chunk(&mut self, min_size: vk::DeviceSize) {
        let chunk = self.chunks.len() as u32;
        let size = align(self.chunk_size(), min_size);
        let mut buffer = DeviceBuffer::new(
            Arc::clone(&self.device),
            size,
            self.usage(),
            self.mapping,
            self.lifetime,
        );
        buffer.binding = Some(self.binding);
        buffer.heap = Weak::clone(&self.heap);
        buffer.set_chunk(chunk);

        buffer.set_name(format!(
            "{:?}|{:?}|{:?}[{}]",
            self.binding, self.lifetime, self.mapping, chunk,
        ));

        self.chunks.push(Arc::new(buffer));
        self.allocator.add_chunk(size);
    }

    fn alloc(&mut self, size: vk::DeviceSize) -> BufferAlloc {
        assert_ne!(size, 0);
        let alignment = self.alignment();
        let orig_size = size;
        let size = align(alignment, size);

        let limits = self.device.limits();
        match self.binding {
            BufferBinding::Uniform => assert!(size < limits.max_uniform_buffer_range as _),
            BufferBinding::Storage => assert!(size < limits.max_storage_buffer_range as _),
            _ => (),
        }

        let block = self
            .allocator
            .alloc(size, alignment)
            .or_else(|| {
                self.add_chunk(size);
                self.allocator.alloc(size, alignment)
            })
            .unwrap();
        let buffer = Arc::clone(&self.chunks[block.chunk as usize]);
        BufferAlloc {
            buffer,
            offset: block.offset(),
            size: orig_size,
        }
    }

    fn free(&mut self, alloc: &BufferAlloc) {
        // Make sure the allocation came from this pool
        let chunk = &self.chunks[alloc.chunk() as usize];
        assert!(
            Arc::ptr_eq(&alloc.buffer, chunk),
            "alloc: {:?},\nself.chunks[alloc.chunk]: {:?}",
            alloc,
            chunk,
        );

        let alignment = self.alignment();
        let adj_size = align(alignment, alloc.size());
        let mut block = to_block(alloc);
        block.end = block.start + adj_size;
        self.allocator.free(block);
    }

    unsafe fn clear(&mut self) {
        for chunk in self.chunks.iter() {
            assert_eq!(
                Arc::strong_count(chunk),
                1,
                "chunk cleared while in use: {:?}",
                chunk
            );
        }
        self.allocator.clear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use more_asserts::assert_ge;
    use vk::traits::*;

    unsafe fn create_buffer(vars: testing::TestVars) {
        DeviceBuffer::new(
            Arc::clone(vars.device()),
            8 * (2 << 20),
            BufferUsage::TRANSFER_SRC,
            MemoryMapping::Mapped,
            Lifetime::Static,
        );
    }

    unsafe fn heap_alloc(vars: testing::TestVars) {
        use BufferBinding::*;
        use Lifetime::*;
        use MemoryMapping::*;

        let heap = Arc::new(BufferHeap::new(Arc::clone(vars.device())));

        let x = heap.boxed(Uniform, Static, [0.0f32, 0.5, 0.5, 1.0]);
        assert_eq!(x[1], 0.5);

        heap.alloc(Uniform, Frame, DeviceLocal, 256);
        heap.clear_frame();
        // TODO: Query used memory
    }

    unsafe fn oversized_alloc(vars: testing::TestVars) {
        let device = Arc::clone(vars.device());
        let heap = Arc::new(BufferHeap::new(Arc::clone(&device)));
        let _ = heap.alloc(
            BufferBinding::Uniform,
            Lifetime::Static,
            MemoryMapping::Mapped,
            (2 * device.limits().max_uniform_buffer_range) as _,
        );
    }

    unsafe fn alloc_size_is_exact(vars: testing::TestVars) {
        use BufferBinding::*;
        use Lifetime::*;
        use MemoryMapping::*;

        let heap = Arc::new(BufferHeap::new(Arc::clone(vars.device())));
        let alloc = heap.alloc(Uniform, Static, Mapped, 15);
        assert_eq!(alloc.size(), 15);
        let alloc = heap.alloc(Uniform, Static, Mapped, 32);
        assert_eq!(alloc.size(), 32);
        let alloc = heap.alloc(Uniform, Static, Mapped, 35);
        assert_eq!(alloc.size(), 35);
    }

    unsafe fn non_overlapping(vars: testing::TestVars) {
        use BufferBinding::*;
        use Lifetime::*;
        use MemoryMapping::*;

        let heap = Arc::new(BufferHeap::new(Arc::clone(vars.device())));
        let alloc0 = heap.alloc(Uniform, Static, Mapped, 513);
        let alloc1 = heap.alloc(Uniform, Static, Mapped, 1024);
        assert_eq!(alloc0.offset(), 0);
        assert_ge!(alloc1.offset(), alloc0.size());
    }

    unsafe fn free(vars: testing::TestVars) {
        use BufferBinding::*;
        use Lifetime::*;
        use MemoryMapping::*;

        let heap = Arc::new(BufferHeap::new(Arc::clone(vars.device())));
        std::mem::drop([
            heap.alloc(Uniform, Static, Mapped, 98),
            heap.alloc(Uniform, Static, Mapped, 99),
            heap.alloc(Uniform, Static, Mapped, 100),
        ]);
        let alloc = heap.alloc(Uniform, Static, Mapped, 32);
        assert_eq!(alloc.offset(), 0);
    }
}
