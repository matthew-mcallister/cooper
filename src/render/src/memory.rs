//! This module defines memory allocators. It is not responsible for
//! populating memory or binding buffers, as that is the resource
//! manager's job.
use std::cmp::Ordering;
use std::error::Error;
use std::ptr;
use std::sync::Arc;

use super::Device;

#[derive(Clone, Copy)]
struct MemoryType<'a> {
    props: &'a vk::PhysicalDeviceMemoryProperties,
    index: u32,
}

impl<'a> MemoryType<'a> {
    fn inner(&self) -> &'a vk::MemoryType
        { &self.props.memory_types[self.index as usize] }

    fn heap(&self) -> &'a vk::MemoryHeap
        { &self.props.memory_heaps[self.inner().heap_index as usize] }

    fn flags(&self) -> vk::MemoryPropertyFlags
        { self.inner().property_flags }

    fn cached(&self) -> bool {
        self.inner().property_flags
            .intersects(vk::MemoryPropertyFlags::HOST_CACHED_BIT)
    }
}

#[derive(Clone, Copy)]
crate struct MemoryAllocateOptions {
    crate required_flags: vk::MemoryPropertyFlags,
}

#[derive(Clone, Copy)]
crate struct MemoryTypeChooser {
    props: vk::PhysicalDeviceMemoryProperties,
}

impl MemoryTypeChooser {
    crate unsafe fn new(rdev: &RenderDevice) -> Self {
        let mut props = Default::default();
        rdev.it().get_physical_device_memory_properties
            (rdev.pdev, &mut props as _);
        MemoryTypeChooser { props }
    }

    fn types(&self) -> impl Iterator<Item = MemoryType<'_>> + '_ {
        (0..self.props.memory_type_count)
            .map(move |index| MemoryType { props: &self.props, index })
    }

    // Finds a desirable memory type that meets requirements.
    crate fn find_type_index(
        &self,
        requirements: vk::MemoryRequirements,
        options: &MemoryAllocateOptions,
    ) -> Option<u32> {
        let compare_types = |ty1: &MemoryType, ty2: &MemoryType| {
            if ty1.heap().size > ty2.heap().size
                { return Ordering::Greater; }
            match (ty1.cached(), ty2.cached()) {
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                _ => Ordering::Equal,
            }
        };
        let res = self.types()
            .filter(|ty| (1 << ty.index) & requirements.memory_type_bits > 0)
            .filter(|ty| ty.flags().contains(options.required_flags))
            .filter(|ty| ty.heap().size >= requirements.size)
            .max_by(compare_types)?
            .index;
        Some(res)
    }
}

// Memory allocator where each allocation results in a call to
// `vkAllocateMemory`. In a future improvement, it will automatically
// use `VK_KHR_dedicated_allocation` if enabled.
crate struct DedicatedMemoryAllocator {
    rdev: Arc<RenderDevice>,
    chooser: MemoryTypeChooser,
}

impl DedicatedMemoryAllocator {
    crate fn dt(&self) -> &vkl::DeviceTable {
        &self.rdev.table
    }

    crate unsafe fn new(rdev: Arc<RenderDevice>) -> Self {
        let chooser = MemoryTypeChooser::new(&rdev);
        DedicatedMemoryAllocator { rdev, chooser }
    }

    crate unsafe fn allocate(
        &self,
        requirements: vk::MemoryRequirements,
        options: &MemoryAllocateOptions,
    ) -> Result<vk::DeviceMemory, Box<dyn Error>> {
        let idx = self.chooser.find_type_index(requirements, options)
            .ok_or("no suitable device memory for allocation")?;
        let alloc_info = vk::MemoryAllocateInfo {
            s_type: vk::StructureType::MEMORY_ALLOCATE_INFO,
            p_next: ptr::null(),
            allocation_size: requirements.size,
            memory_type_index: idx,
        };
        let mut memory = vk::null();
        self.dt().allocate_memory
            (&alloc_info as _, ptr::null(), &mut memory as _).check()?;
        Ok(memory)
    }

    crate unsafe fn create_buffer(
        &self,
        create_info: &vk::BufferCreateInfo,
        options: &MemoryAllocateOptions,
    ) -> Result<(vk::Buffer, vk::DeviceMemory), Box<dyn Error>> {
        let mut buf = vk::null();
        let mut memory = vk::null();
        let res: Result<(), Box<dyn Error>> = try {
            self.dt().create_buffer
                (create_info as _, ptr::null(), &mut buf as _).check()?;

            let mut reqs = Default::default();
            self.dt().get_buffer_memory_requirements(buf, &mut reqs as _);
            memory = self.allocate(reqs, options)?;

            self.dt().bind_buffer_memory(buf, memory, 0).check()?;
        };
        if let Err(e) = res {
            self.dt().free_memory(memory, ptr::null());
            self.dt().destroy_buffer(buf, ptr::null());
            Err(e)?;
        }
        Ok((buf, memory))
    }

    crate unsafe fn create_image(
        &self,
        create_info: &vk::ImageCreateInfo,
        options: &MemoryAllocateOptions,
    ) -> Result<(vk::Image, vk::DeviceMemory), Box<dyn Error>> {
        let mut image = vk::null();
        let mut memory = vk::null();
        let res: Result<(), Box<dyn Error>> = try {
            self.dt().create_image
                (create_info as _, ptr::null(), &mut image as _).check()?;

            let mut reqs = Default::default();
            self.dt().get_image_memory_requirements(image, &mut reqs as _);
            memory = self.allocate(reqs, options)?;

            self.dt().bind_image_memory(image, memory, 0).check()?;
        };
        if let Err(e) = res {
            self.dt().free_memory(memory, ptr::null());
            self.dt().destroy_image(image, ptr::null());
            Err(e)?;
        }
        Ok((image, memory))
    }

    crate unsafe fn copy_to_buffer(
        &self,
        usage: vk::BufferUsageFlags,
        src: &[u8],
    ) -> Result<(vk::Buffer, vk::DeviceMemory), Box<dyn Error>> {
        let create_info = vk::BufferCreateInfo {
            s_type: vk::StructureType::BUFFER_CREATE_INFO,
            p_next: ptr::null(),
            flags: Default::default(),
            size: src.len() as _,
            usage,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            queue_family_index_count: 0,
            p_queue_family_indices: ptr::null(),
        };
        let options = MemoryAllocateOptions {
            required_flags: vk::MemoryPropertyFlags::HOST_VISIBLE_BIT,
        };
        let (buf, mem) = self.create_buffer(&create_info, &options)?;
        if let Err(e) = map_copy_unmap(&self.rdev, src, mem) {
            self.dt().destroy_buffer(buf, ptr::null());
            self.dt().free_memory(mem, ptr::null());
            Err(e)?;
            unreachable!();
        } else {
            Ok((buf, mem))
        }
    }
}

crate unsafe fn map_copy_unmap(
    rdev: &RenderDevice,
    src: &[u8],
    memory: vk::DeviceMemory,
) -> Result<(), vk::Result> {
    let mut dst = ptr::null_mut();
    rdev.table.map_memory(
        memory,
        0,
        vk::WHOLE_SIZE,
        Default::default(),
        &mut dst as *mut _ as _,
    ).check()?;
    ptr::copy_nonoverlapping(src.as_ptr(), dst, src.len());
    let range = vk::MappedMemoryRange {
        s_type: vk::StructureType::MAPPED_MEMORY_RANGE,
        p_next: ptr::null(),
        memory,
        offset: 0,
        size: vk::WHOLE_SIZE,
    };
    // TODO: Skip flush if memory is coherent
    let res = rdev.table.flush_mapped_memory_ranges(1, &range as _);
    rdev.table.unmap_memory(memory);
    res.check()?;
    Ok(())
}

// # Usage notes
//
// - The `initial_layout` field of the `create_info` parameter is
//   ignored, as it must be `VK_IMAGE_LAYOUT_UNDEFINED` anyways.
// - The final layout of the image will be SHADER_READ_ONLY_OPTIMAL.
// - The `usage` field of the `create_info` parameter will automatically
//   have the `TRANSFER_DST_BIT` set; it may be omitted by the caller.
crate unsafe fn upload_image(
    renderer: &Renderer,
    img_create_info: &vk::ImageCreateInfo,
    // NB: Could replace with impl Read + Seek
    data: &[u8],
) -> Result<(vk::Image, vk::DeviceMemory), Box<dyn Error>> {
    let rnd = renderer;
    let allocator = &rnd.allocator;

    let new_create_info = vk::ImageCreateInfo {
        usage: img_create_info.usage | vk::ImageUsageFlags::TRANSFER_DST_BIT,
        initial_layout: vk::ImageLayout::UNDEFINED,
        ..*img_create_info
    };
    let options = MemoryAllocateOptions {
        required_flags: vk::MemoryPropertyFlags::DEVICE_LOCAL_BIT,
    };
    let (img, img_mem) = allocator.create_image(&new_create_info, &options)?;

    let (mut buf, mut buf_mem) = (vk::null(), vk::null());
    let mut cmd_buf = vk::null();
    let mut fence = vk::null();
    let res: Result<(), Box<dyn Error>> = try {
        let (buf_, buf_mem_) = allocator.copy_to_buffer
            (vk::BufferUsageFlags::TRANSFER_SRC_BIT, data)?;
        buf = buf_;
        buf_mem = buf_mem_;

        cmd_buf = renderer.allocate_command_buffer()?;

        rnd.dt.begin_command_buffer(cmd_buf, &vk::CommandBufferBeginInfo {
            s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
            p_next: ptr::null(),
            flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT_BIT,
            p_inheritance_info: ptr::null(),
        } as _);

        let barrier = vk::ImageMemoryBarrier {
            s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
            p_next: ptr::null(),
            src_access_mask: vk::AccessFlags::empty(),
            dst_access_mask: vk::AccessFlags::TRANSFER_WRITE_BIT,
            old_layout: vk::ImageLayout::UNDEFINED,
            new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            image: img,
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR_BIT,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            },
        };
        rnd.dt.cmd_pipeline_barrier(
            cmd_buf,
            vk::PipelineStageFlags::TOP_OF_PIPE_BIT,
            vk::PipelineStageFlags::TRANSFER_BIT,
            Default::default(),
            0, ptr::null(),
            0, ptr::null(),
            1, &barrier as _,
        );

        let region = vk::BufferImageCopy {
            buffer_offset: 0,
            buffer_row_length: 0,
            buffer_image_height: 0,
            image_subresource: vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR_BIT,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            },
            image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
            image_extent: img_create_info.extent,
        };
        rnd.dt.cmd_copy_buffer_to_image(
            cmd_buf,
            buf,
            img,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            1,
            &region as _,
        );

        let barrier = vk::ImageMemoryBarrier {
            src_access_mask: vk::AccessFlags::TRANSFER_WRITE_BIT,
            dst_access_mask: vk::AccessFlags::SHADER_READ_BIT,
            old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            ..barrier
        };
        rnd.dt.cmd_pipeline_barrier(
            cmd_buf,
            vk::PipelineStageFlags::TRANSFER_BIT,
            vk::PipelineStageFlags::FRAGMENT_SHADER_BIT,
            Default::default(),
            0, ptr::null(),
            0, ptr::null(),
            1, &barrier as _,
        );

        rnd.dt.end_command_buffer(cmd_buf).check()?;

        let create_info = vk::FenceCreateInfo {
            s_type: vk::StructureType::FENCE_CREATE_INFO,
            ..Default::default()
        };
        rnd.dt.create_fence
            (&create_info as _, ptr::null(), &mut fence as _)
            .check()?;

        let submit_info = vk::SubmitInfo {
            s_type: vk::StructureType::SUBMIT_INFO,
            command_buffer_count: 1,
            p_command_buffers: &cmd_buf as _,
            ..Default::default()
        };
        rnd.dt.queue_submit(rnd.queue(), 1, &submit_info as _, fence)
            .check()?;

        // We wait for the fence here, but in practice we would want to
        // defer until immediately before rendering.
        rnd.dt.wait_for_fences
            (1, &fence as _, vk::TRUE, !0)
            .check()?;
    };

    rnd.dt.destroy_fence(fence, ptr::null());
    rnd.dt.free_command_buffers(renderer.cmd_pool, 1, &cmd_buf as _);
    rnd.dt.destroy_buffer(buf, ptr::null());
    rnd.dt.free_memory(buf_mem, ptr::null());
    if let Err(e) = res {
        rnd.dt.destroy_image(img, ptr::null());
        rnd.dt.free_memory(img_mem, ptr::null());
        Err(e)?;
        unreachable!();
    } else {
        Ok((img, img_mem))
    }
}
