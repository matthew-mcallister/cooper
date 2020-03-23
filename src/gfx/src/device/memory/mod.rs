use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::ptr;
use std::sync::Arc;

use enum_map::Enum;
use log::{debug, trace};

use crate::*;

mod alloc;
mod heap;
mod buffer;

pub(self) use alloc::*;
crate use heap::*;
crate use buffer::*;

#[derive(Clone, Copy, Debug, Default)]
struct Block {
    chunk: u32,
    start: vk::DeviceSize,
    end: vk::DeviceSize,
}

#[derive(Debug)]
crate struct DeviceMemory {
    device: Arc<Device>,
    inner: vk::DeviceMemory,
    size: vk::DeviceSize,
    type_index: u32,
    ptr: *mut c_void,
    tiling: Tiling,
    dedicated_content: Option<DedicatedAllocContent>,
    chunk: u32,
}

#[derive(Clone, Copy, Debug, Enum, Eq, Hash, PartialEq)]
crate enum Tiling {
    /// Denotes a linear image or a buffer.
    Linear,
    /// Denotes a nonlinear (a.k.a. optimal) image.
    Nonlinear,
}

repr_bool! {
    crate enum MemoryMapping {
        Unmapped = false,
        Mapped = true,
    }
}

/// Tells how long memory or other resources live for.
#[derive(Clone, Copy, Debug, Enum, Eq, Hash, PartialEq)]
pub enum Lifetime {
    // Lives until freed or destroyed.
    Static,
    /// Lives at least the duration of a frame.
    Frame,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
crate enum DedicatedAllocContent {
    Image(vk::Image),
    Buffer(vk::Buffer),
}

#[derive(Clone, Copy, Debug, Default)]
crate struct HeapInfo {
    reserved: vk::DeviceSize,
    used: vk::DeviceSize,
}

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

fn find_memory_type(
    device: &Device,
    flags: vk::MemoryPropertyFlags,
    type_mask: u32,
) -> Option<u32> {
    // According to the spec, implementations are to sort memory types
    // in order of "performance", so the first memory type with the
    // required properties is probably the best for general use.
    iter_memory_types(device)
        .enumerate()
        .filter(|&(idx, ty)| {
            compatible_type(type_mask, idx as u32)
                && ty.property_flags.contains(flags)
        })
        .next()
        .map(|(idx, _)| idx as u32)
}

fn find_memory_type_2(
    device: &Device,
    mapping: MemoryMapping,
    reqs: &vk::MemoryRequirements,
) -> Option<u32> {
    find_memory_type(
        device,
        mapping.memory_property_flags(),
        reqs.memory_type_bits,
    )
}

#[inline(always)]
crate fn visible_coherent_flags() -> vk::MemoryPropertyFlags {
    vk::MemoryPropertyFlags::HOST_VISIBLE_BIT |
        vk::MemoryPropertyFlags::HOST_COHERENT_BIT
}

unsafe fn alloc_device_memory(
    device: &Device,
    alloc_info: &vk::MemoryAllocateInfo,
) -> vk::DeviceMemory {
    trace!("allocating device memory: size: {}, type: {}",
        alloc_info.allocation_size, alloc_info.memory_type_index);
    let dt = &*device.table;
    let mut memory = vk::null();
    dt.allocate_memory(alloc_info, ptr::null(), &mut memory).check()
        .unwrap_or_else(|_|
            panic!("failed to allocate device memory: {:?}", alloc_info));
    memory
}

unsafe fn alloc_resource_memory(
    device: Arc<Device>,
    mapping: MemoryMapping,
    reqs: &vk::MemoryRequirements,
    content: Option<DedicatedAllocContent>,
    tiling: Tiling,
) -> DeviceMemory {
    use DedicatedAllocContent::*;

    let mut p_next = ptr::null_mut();

    let mut dedicated = vk::MemoryDedicatedAllocateInfo::default();
    if let Some(content) = content {
        add_to_pnext!(p_next, dedicated);
        match content {
            Buffer(buffer) => dedicated.buffer = buffer,
            Image(image) => dedicated.image = image,
        }
    }

    let type_index = find_memory_type_2(&device, mapping, reqs).unwrap();
    let alloc_info = vk::MemoryAllocateInfo {
        p_next,
        allocation_size: reqs.size,
        memory_type_index: type_index,
        ..Default::default()
    };

    if let Some(content) = content {
        debug!("creating dedicated allocation: size: {:?}, type: {:?}, {:?}",
            reqs.size, type_index, content);
    }

    let inner = alloc_device_memory(&device, &alloc_info);

    // Fill out boilerplate
    let mut memory = DeviceMemory {
        device,
        inner,
        size: reqs.size,
        type_index,
        ptr: ptr::null_mut(),
        tiling,
        dedicated_content: content,
        // Caller should fill this out
        chunk: !0,
    };
    memory.init();
    memory
}

unsafe fn get_buffer_memory_reqs(device: &Device, buffer: vk::Buffer) ->
    (vk::MemoryRequirements, vk::MemoryDedicatedRequirements)
{
    let dt = &*device.table;
    let mut dedicated_reqs = vk::MemoryDedicatedRequirements::default();
    let mut reqs = vk::MemoryRequirements2 {
        p_next: &mut dedicated_reqs as *mut _ as _,
        ..Default::default()
    };
    let buffer_info = vk::BufferMemoryRequirementsInfo2 {
        buffer,
        ..Default::default()
    };
    dt.get_buffer_memory_requirements_2(&buffer_info, &mut reqs);
    (reqs.memory_requirements, dedicated_reqs)
}

unsafe fn get_image_memory_reqs(device: &Device, image: vk::Image) ->
    (vk::MemoryRequirements, vk::MemoryDedicatedRequirements)
{
    let dt = &*device.table;
    let mut dedicated_reqs = vk::MemoryDedicatedRequirements::default();
    let mut reqs = vk::MemoryRequirements2 {
        p_next: &mut dedicated_reqs as *mut _ as _,
        ..Default::default()
    };
    let image_info = vk::ImageMemoryRequirementsInfo2 {
        image,
        ..Default::default()
    };
    dt.get_image_memory_requirements_2(&image_info, &mut reqs);
    (reqs.memory_requirements, dedicated_reqs)
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

    fn as_ptr<T>(&self) -> *mut MaybeUninit<T> {
        let ptr = self.as_raw() as *mut MaybeUninit<T>;
        assert_eq!(ptr as usize % std::mem::align_of::<T>(), 0);
        ptr
    }

    fn as_mut<T>(&mut self) -> &mut MaybeUninit<T> {
        assert!(std::mem::size_of::<T>() as vk::DeviceSize <= self.size());
        unsafe { &mut *self.as_ptr::<T>() }
    }

    fn as_mut_slice<T>(&mut self, len: usize) -> &mut [MaybeUninit<T>] {
        let ptr = self.as_ptr::<T>();
        assert!(self.size() as usize >= len * std::mem::size_of::<T>());
        unsafe { std::slice::from_raw_parts_mut(ptr, len) }
    }
}

fn to_block<T: MemoryRegion>(region: &T) -> Block {
    Block {
        chunk: region.memory().chunk,
        start: region.offset(),
        end: region.end(),
    }
}

unsafe impl Send for DeviceMemory {}
unsafe impl Sync for DeviceMemory {}

impl Drop for DeviceMemory {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe { dt.free_memory(self.inner, ptr::null()); }
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

    crate fn mapped(&self) -> bool {
        !self.ptr.is_null()
    }

    crate fn flags(&self) -> vk::MemoryPropertyFlags {
        self.device.mem_props.memory_types[self.type_index as usize]
            .property_flags
    }

    unsafe fn init(&mut self) {
        if self.flags().contains(vk::MemoryPropertyFlags::HOST_VISIBLE_BIT) {
            self.map();
        }
    }

    unsafe fn map(&mut self) {
        assert!(self.ptr.is_null());
        let dt = &*self.device.table;
        let flags = Default::default();
        dt.map_memory(self.inner, 0, self.size, flags, &mut self.ptr)
            .check().expect("failed to map device memory");
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

impl MemoryMapping {
    crate fn memory_property_flags(self) -> vk::MemoryPropertyFlags {
        match self {
            Self::Mapped => visible_coherent_flags(),
            Self::Unmapped => vk::MemoryPropertyFlags::DEVICE_LOCAL_BIT,
        }
    }

    crate fn usage(self) -> vk::BufferUsageFlags {
        match self {
            Self::Unmapped => vk::BufferUsageFlags::TRANSFER_DST_BIT,
            Self::Mapped => Default::default(),
        }
    }

    crate fn mapped(self) -> bool {
        self.into()
    }
}

impl DedicatedAllocContent {
    fn tiling(self) -> Tiling {
        match self {
            Self::Buffer(_) => Tiling::Linear,
            Self::Image(_) => Tiling::Nonlinear,
        }
    }
}

unit::collect_tests![heap, buffer];
