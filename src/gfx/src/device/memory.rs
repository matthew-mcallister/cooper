use std::ops::Range;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::mem::{ManuallyDrop, MaybeUninit};
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

crate fn find_memory_type_reqs(
    device: &Device,
    flags: vk::MemoryPropertyFlags,
    reqs: &vk::MemoryRequirements,
) -> Option<u32> {
    find_memory_type(device, flags, reqs.memory_type_bits)
}

#[inline(always)]
crate fn visible_coherent_flags() -> vk::MemoryPropertyFlags {
    vk::MemoryPropertyFlags::HOST_VISIBLE_BIT |
        vk::MemoryPropertyFlags::HOST_COHERENT_BIT
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

/// A suballocation of a VkMemory object.
#[derive(Clone, Debug)]
crate struct DeviceRange {
    memory: Arc<DeviceMemory>,
    offset: vk::DeviceSize,
    // N.B. The allocator may return a larger allocation than requested
    size: vk::DeviceSize,
}

crate type DeviceAlloc = DeviceRange;

#[derive(Clone, Copy, Debug, Enum, Eq, PartialEq)]
crate enum Tiling {
    /// Denotes a linear image or a buffer.
    Linear,
    /// Denotes a nonlinear (a.k.a. optimal) image.
    Nonlinear,
}

crate trait MemoryRegion {
    fn memory(&self) -> &Arc<DeviceMemory>;
    fn offset(&self) -> vk::DeviceSize;
    fn size(&self) -> vk::DeviceSize;

    fn end(&self) -> vk::DeviceSize {
        self.offset() + self.size()
    }

    fn as_raw(&self) -> *mut c_void {
        assert!(!self.memory().ptr.is_null());
        unsafe { self.memory().ptr.add(self.offset() as _) }
    }

    fn as_ptr<T>(&self) -> *mut T {
        let ptr = self.as_raw() as *mut T;
        assert_eq!(ptr as usize % std::mem::align_of::<T>(), 0);
        ptr
    }

    fn as_mut<T>(&mut self) -> &mut T {
        assert!(std::mem::size_of::<T>() as vk::DeviceSize <= self.size());
        unsafe { &mut *self.as_ptr::<T>() }
    }

    fn as_mut_slice<T>(&mut self) -> &mut [T] {
        let ptr = self.as_ptr::<T>();
        let elem_size = std::mem::size_of::<T>();
        let len = self.size() as usize / elem_size;
        unsafe { std::slice::from_raw_parts_mut(ptr, len) }
    }
}

fn get_block<T: MemoryRegion>(region: &T) -> Block {
    Block {
        chunk: region.memory().chunk,
        start: region.offset(),
        end: region.end(),
    }
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

unsafe fn alloc_device_memory(
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

impl MemoryRegion for DeviceRange {
    fn memory(&self) -> &Arc<DeviceMemory> {
        &self.memory
    }

    fn offset(&self) -> vk::DeviceSize {
        self.offset
    }

    fn size(&self) -> vk::DeviceSize {
        self.size
    }
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

#[derive(Clone, Debug)]
crate struct DeviceBuffer {
    memory: Arc<DeviceMemory>,
    inner: vk::Buffer,
    usage: vk::BufferUsageFlags,
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
crate struct BufferData<T: ?Sized> {
    alloc: BufferRange,
    ptr: *mut T,
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

impl<T: ?Sized> Drop for BufferData<T> {
    fn drop(&mut self) {
        unsafe { std::ptr::drop_in_place(self.ptr); }
    }
}

impl<T: ?Sized> From<BufferData<T>> for BufferRange {
    fn from(data: BufferData<T>) -> Self {
        data.into_inner()
    }
}

impl<T: ?Sized> BufferData<T> {
    crate fn alloc(&self) -> &BufferRange {
        &self.alloc
    }

    crate fn into_inner(self) -> BufferRange {
        let alloc = unsafe { ptr::read(&self.alloc) };
        std::mem::forget(self);
        alloc
    }
}

impl<T> BufferData<MaybeUninit<T>> {
    crate fn new(mut alloc: BufferRange) -> Self {
        let ptr = alloc.as_mut::<MaybeUninit<T>>() as *mut _;
        BufferData { alloc, ptr }
    }

    crate unsafe fn assume_init(this: Self) -> BufferData<T> {
        let ptr = std::mem::transmute(this.ptr);
        BufferData {
            alloc: this.into_inner(),
            ptr,
        }
    }
}

impl<T> BufferData<[MaybeUninit<T>]> {
    crate fn new_slice(mut alloc: BufferRange) -> Self {
        let ptr = alloc.as_mut_slice::<MaybeUninit<T>>() as *mut _;
        BufferData { alloc, ptr }
    }

    crate unsafe fn assume_init_slice(this: Self) -> BufferData<[T]> {
        let ptr = std::mem::transmute(this.ptr);
        BufferData {
            alloc: this.into_inner(),
            ptr,
        }
    }
}

impl<T: ?Sized> std::ops::Deref for BufferData<T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<T: ?Sized> std::ops::DerefMut for BufferData<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Block {
    chunk: u32,
    start: vk::DeviceSize,
    end: vk::DeviceSize,
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

    fn alloc(
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

#[derive(Clone, Copy, Debug, Default)]
crate struct HeapInfo {
    reserved: vk::DeviceSize,
    used: vk::DeviceSize,
}

// TODO maybe: Multithread this so all memory can be RAII
#[derive(Debug)]
crate struct DeviceHeap {
    device: Arc<Device>,
    // One pool per memory type per tiling
    pools: Vec<EnumMap<Tiling, HeapPool>>,
}

impl Drop for HeapPool {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            for chunk in self.chunks.iter() {
                assert_eq!(Arc::strong_count(chunk), 1,
                    "allocation freed while still in use: {:?}", chunk);
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
        self.allocator.free(get_block(&alloc));
    }

    fn clear(&mut self) {
        self.allocator.clear()
    }
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
        DeviceHeap {
            device,
            pools,
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

    crate fn heaps(&self) -> Vec<HeapInfo> {
        let heap_count = self.device.mem_props.memory_heap_count as usize;
        let mut heaps = vec![HeapInfo::default(); heap_count];
        for pool in self.pools.iter().flat_map(|x| x.values()) {
            let heap = &mut heaps[pool.heap_index() as usize];
            heap.reserved += pool.reserved();
            heap.used += pool.used();
        }
        heaps
    }

    crate unsafe fn alloc(
        &mut self,
        reqs: vk::MemoryRequirements,
        tiling: Tiling,
        mapping: MemoryMapping,
    ) -> DeviceAlloc {
        // TODO: fall back to incoherent memory on failure
        let type_idx = find_memory_type(
            &*self.device,
            mapping.memory_property_flags(),
            reqs.memory_type_bits,
        ).unwrap();
        self.pool_mut(type_idx, tiling).alloc(reqs.size, reqs.alignment)
    }

    /// Binds a buffer to newly allocated memory.
    crate unsafe fn alloc_buffer_memory(
        &mut self,
        buffer: vk::Buffer,
        mapping: MemoryMapping,
    ) -> DeviceAlloc {
        let mut reqs = Default::default();
        self.dt().get_buffer_memory_requirements(buffer, &mut reqs);
        let alloc = self.alloc(reqs, Tiling::Linear, mapping);

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
        mapping: MemoryMapping,
    ) -> DeviceAlloc {
        let mut reqs = Default::default();
        self.dt().get_image_memory_requirements(image, &mut reqs);
        let alloc = self.alloc(reqs, Tiling::Nonlinear, mapping);

        let memory = alloc.memory().inner();
        let offset = alloc.offset();
        self.dt().bind_image_memory(image, memory, offset).check().unwrap();

        alloc
    }

    // TODO: Allocations might be freed via RAII
    crate fn free(&mut self, alloc: DeviceAlloc) {
        let type_index = alloc.memory().type_index();
        let tiling = alloc.memory().tiling();
        self.pool_mut(type_index, tiling).free(alloc);
    }
}

#[derive(Debug)]
crate struct BufferPool {
    device: Arc<Device>,
    mapping: MemoryMapping,
    usage: vk::BufferUsageFlags,
    allocator: FreeListAllocator,
    chunks: Vec<Arc<DeviceBuffer>>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
crate enum MemoryMapping {
    Mapped,
    Unmapped,
}

impl Drop for BufferPool {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            for chunk in self.chunks.iter() {
                assert_eq!(Arc::strong_count(chunk), 1,
                    "buffer destroyed while still in use: {:?}", chunk);
                dt.destroy_buffer(chunk.inner, ptr::null());
                dt.free_memory(chunk.memory.inner, ptr::null());
            }
        }
    }
}

impl BufferPool {
    crate fn new(
        device: Arc<Device>,
        mapping: MemoryMapping,
        usage: vk::BufferUsageFlags,
    ) -> Self {
        Self {
            device,
            mapping,
            usage,
            allocator: Default::default(),
            chunks: Vec::new(),
        }
    }

    // TODO: Refactor overlap with HeapPool type
    crate fn used(&self) -> vk::DeviceSize {
        self.allocator.used()
    }

    crate fn reserved(&self) -> vk::DeviceSize {
        self.allocator.capacity()
    }

    fn chunk_size(&self) -> vk::DeviceSize {
        0x100_0000
    }

    fn min_alignment(&self) -> vk::DeviceSize {
        32
    }

    crate fn mapping(&self) -> MemoryMapping {
        self.mapping
    }

    crate fn mapped(&self) -> bool {
        self.mapping.into()
    }

    unsafe fn add_chunk(&mut self, min_size: vk::DeviceSize) {
        let dt = &*self.device.table;

        let chunk = self.chunks.len() as u32;
        let size = align(self.chunk_size(), min_size);

        let create_info = vk::BufferCreateInfo {
            size,
            usage: self.usage,
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
            inner: memory,
            size,
            type_index,
            ptr: 0 as _,
            tiling: Tiling::Linear,
            chunk,
        };
        if self.mapped() { mem.map(&self.device); }

        let buffer = DeviceBuffer {
            memory: Arc::new(mem),
            inner: buffer,
            usage: self.usage,
        };

        self.chunks.push(Arc::new(buffer));
        self.allocator.add_chunk(size);
    }

    crate fn alloc(&mut self, size: vk::DeviceSize, alignment: vk::DeviceSize)
        -> BufferRange
    {
        let alignment = std::cmp::max(self.min_alignment(), alignment);
        let size = align(alignment, size);
        assert_ne!(size, 0);
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

    crate fn alloc_block<T>(&mut self) -> BufferData<MaybeUninit<T>> {
        assert!(self.mapped());
        let alloc = self.alloc(
            std::mem::size_of::<T>() as _,
            // N.B. alignment is rounded to 32 so we *shouldn't* hit
            // alignment issues with (d)vec4, but beware of surprises.
            std::mem::align_of::<T>() as _,
        );
        BufferData::new(alloc)
    }

    crate fn alloc_array<T>(&mut self, len: usize) ->
        BufferData<[MaybeUninit<T>]>
    {
        assert!(self.mapped());
        let alloc = self.alloc(
            (std::mem::size_of::<T>() * len) as _,
            std::mem::align_of::<T>() as _,
        );
        BufferData::new_slice(alloc)
    }

    crate fn free(&mut self, alloc: BufferRange) {
        // Make sure the allocation came from this pool
        assert!(Arc::ptr_eq(
            &alloc.buffer,
            &self.chunks[alloc.chunk() as usize],
        ));
        self.allocator.free(get_block(&alloc));
    }

    /// Invalidates existing allocations.
    crate unsafe fn clear(&mut self) {
        self.allocator.clear()
    }
}

impl MemoryMapping {
    fn memory_property_flags(self) -> vk::MemoryPropertyFlags {
        use MemoryMapping::*;
        match self {
            Mapped => visible_coherent_flags(),
            Unmapped => vk::MemoryPropertyFlags::DEVICE_LOCAL_BIT,
        }
    }
}

impl From<MemoryMapping> for bool {
    fn from(mapping: MemoryMapping) -> Self {
        match mapping {
            MemoryMapping::Mapped => true,
            MemoryMapping::Unmapped => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use vk::traits::*;
    use super::*;

    unsafe fn device_heap_smoke(vars: testing::TestVars) {
        use Tiling::*;
        use MemoryMapping::*;

        let device = Arc::clone(&vars.swapchain.device);
        let mut heap = DeviceHeap::new(Arc::clone(&device));

        let reqs = vk::MemoryRequirements {
            size: 4096,
            alignment: 256,
            memory_type_bits: !0,
        };
        let _alloc0 = heap.alloc(reqs, Linear, Mapped);
        let _alloc1 = heap.alloc(reqs, Nonlinear, Unmapped);
        assert_ne!(_alloc0.as_raw(), 0 as _);
        heap.free(_alloc0);
    }

    unsafe fn buffer_pool_smoke(vars: testing::TestVars) {
        let device = Arc::clone(&vars.swapchain.device);
        let mut heap = BufferPool::new(
            device,
            MemoryMapping::Mapped,
            vk::BufferUsageFlags::STORAGE_BUFFER_BIT,
        );

        let mut x = BufferData::assume_init(heap.alloc_block::<[f32; 4]>());
        *x = [0.0, 1.0, 2.0, 3.0];
        assert_eq!(x[1], 1.0);
        heap.free(x.into());
    }

    unit::declare_tests![
        device_heap_smoke,
        buffer_pool_smoke,
    ];
}

unit::collect_tests![tests];
