use std::ptr;
use std::sync::Arc;

use crate::*;

pub type ImageId = u32;

const STAGING_BASE_SIZE: vk::DeviceSize = 0x200_0000;
const DEVICE_BASE_SIZE: vk::DeviceSize = 0x200_0000;

const MAX_XFER_SIZE: vk::DeviceSize = 0x80_0000;

#[derive(Debug)]
pub struct ImageManager {
    device: Arc<Device>,
    path: Arc<RenderPath>,
    gfx_queue: vk::Queue,
    gfx_qf: u32,
    images: Vec<ImageState>,
    // VRAM image storage
    image_mem: Box<MemoryPool>,
    // Intermediate area for uploads; also potentially a main host cache
    // for on-device memory. Caching is worthless on UMA, in which case
    // its size should be kept minimal.
    staging_buf: Box<MemoryPool>,
    xfer_state: Vec<XferState>,
    submission_id: u32,
    // Huge descriptor set for bindless textures
    desc_set: vk::DescriptorSet,
    // Crutch for development
    universal_sampler: vk::Sampler,
}

#[derive(Debug)]
struct ImageState {
    inner: vk::Image,
    view: vk::ImageView,
    extent: vk::Extent2D,
    format: vk::Format,
    staging_alloc: Option<Box<DeviceAlloc>>,
    // ID of the queue submission in which this texture was last updated
    submission_id: Option<u32>,
    bound_alloc: Option<Box<DeviceAlloc>>,
}

impl Drop for ImageManager {
    fn drop(&mut self) {
        for image in self.images.iter() {
            unsafe {
                self.device.table.destroy_image_view(image.view, ptr::null());
                self.device.table.destroy_image(image.inner, ptr::null());
            }
        }
    }
}

unsafe fn create_image_mem(device: Arc<Device>) -> Box<MemoryPool> {
    let mem_flags = vk::MemoryPropertyFlags::DEVICE_LOCAL_BIT;
    let type_index = find_memory_type(&device, mem_flags).unwrap();
    let create_info = MemoryPoolCreateInfo {
        type_index,
        base_size: DEVICE_BASE_SIZE,
        ..Default::default()
    };
    Box::new(MemoryPool::new(device, create_info))
}

unsafe fn create_staging_buf(device: Arc<Device>) -> Box<MemoryPool> {
    let mem_flags = vk::MemoryPropertyFlags::HOST_VISIBLE_BIT;
    let type_index = find_memory_type(&device, mem_flags).unwrap();
    let create_info = MemoryPoolCreateInfo {
        type_index,
        base_size: STAGING_BASE_SIZE,
        host_mapped: true,
        buffer_map_opts: Some(BufferMapOptions {
            usage: vk::BufferUsageFlags::TRANSFER_SRC_BIT,
        }),
        ..Default::default()
    };
    Box::new(MemoryPool::new(device, create_info))
}

unsafe fn create_universal_sampler(res: &mut InitResources) -> vk::Sampler {
    let create_info = Default::default();
    res.objs.create_sampler(&create_info)
}

#[inline]
fn base_image_range() -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange {
        aspect_mask: vk::ImageAspectFlags::COLOR_BIT,
        base_mip_level: 0,
        level_count: 1,
        base_array_layer: 0,
        layer_count: 1,
    }
}

macro_rules! xfer_state {
    ($self:expr) => {{
        let idx =
            ($self.submission_id as usize % $self.xfer_state.len()) as usize;
        &mut $self.xfer_state[idx]
    }};
}

impl ImageManager {
    pub unsafe fn new(path: Arc<RenderPath>, res: &mut InitResources) -> Self {
        let device = Arc::clone(&res.objs.device);

        let images = Default::default();
        let image_mem = create_image_mem(Arc::clone(&device));

        let gfx_qf = 0;
        let gfx_queue = device.get_queue(gfx_qf, 0);

        let staging_buf = create_staging_buf(Arc::clone(&device));
        let xfer_state = XferState::new(res, gfx_qf, 2);

        let desc_set = path.texture_set;

        let universal_sampler = create_universal_sampler(res);

        ImageManager {
            device,
            path,
            images,
            image_mem,
            gfx_queue,
            gfx_qf,
            staging_buf,
            xfer_state,
            submission_id: 0,
            desc_set,
            universal_sampler,
        }
    }

    pub unsafe fn add_image(
        &mut self,
        extent: vk::Extent2D,
        format: vk::Format,
    ) -> u32 {
        let dst_set = self.path.texture_set;
        let slot = self.images.len() as _;
        assert!(slot < self.path.max_texture_descriptors);

        // Create image/view
        let create_info = vk::ImageCreateInfo {
            image_type: vk::ImageType::_2D,
            format,
            extent: vk::Extent3D::new(extent.width, extent.height, 1),
            mip_levels: 1,
            array_layers: 1,
            samples: vk::SampleCountFlags::_1_BIT,
            usage: vk::ImageUsageFlags::TRANSFER_DST_BIT
                | vk::ImageUsageFlags::SAMPLED_BIT,
            initial_layout: vk::ImageLayout::UNDEFINED,
            ..Default::default()
        };
        let mut image = vk::null();
        self.device.table.create_image
            (&create_info as _, ptr::null(), &mut image as _)
            .check().unwrap();

        let alloc = self.image_mem.alloc_image_memory(image);

        let view_create_info = vk::ImageViewCreateInfo {
            image,
            view_type: vk::ImageViewType::_2D,
            format,
            subresource_range: base_image_range(),
            ..Default::default()
        };
        let mut view = vk::null();
        self.device.table.create_image_view
            (&view_create_info as _, ptr::null(), &mut view as _)
            .check().unwrap();

        // Write descriptor
        // TODO: Batch writes?
        let image_infos = [vk::DescriptorImageInfo {
            sampler: self.universal_sampler,
            image_view: view,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        }];
        let writes = [vk::WriteDescriptorSet {
            dst_set,
            dst_binding: 0,
            dst_array_element: slot,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            p_image_info: image_infos.as_ptr(),
            ..Default::default()
        }];
        self.device.table.update_descriptor_sets(
            writes.len() as _,
            writes.as_ptr(),
            0,
            ptr::null(),
        );

        self.images.push(ImageState {
            inner: image,
            view,
            extent,
            format,
            staging_alloc: None,
            submission_id: None,
            bound_alloc: Some(Box::new(alloc)),
        });

        slot
    }

    pub unsafe fn upload_data(&mut self, id: ImageId, bytes: &[u8]) {
        let xfer = xfer_state!(self);

        if xfer.size + bytes.len() >= MAX_XFER_SIZE as _ {
            self.next_xfer();
        }

        let img = &mut self.images[id as usize];
        img.submission_id = Some(self.submission_id);

        let reqs = vk::MemoryRequirements {
            size: bytes.len() as _,
            alignment: 16,
            memory_type_bits: !0,
        };
        let alloc = self.staging_buf.allocate(&reqs);
        img.staging_alloc = Some(Box::new(alloc));

        // TODO: Periodically empty staging buffer on UMA
        let slice = &mut *alloc.info().as_slice();
        slice.copy_from_slice(bytes);

        let xfer = xfer_state!(self);
        xfer.emit_image_copy(img);
    }

    unsafe fn next_xfer(&mut self) {
        xfer_state!(self).start_xfer();
        self.submission_id += 1;
        xfer_state!(self).flush(self.submission_id);
    }

    pub unsafe fn flush(&mut self) {
        xfer_state!(self).flush(self.submission_id);
        self.submission_id += 1;
        xfer_state!(self).flush(self.submission_id);
    }
}

#[inline]
fn begin_one_time() -> vk::CommandBufferBeginInfo {
    vk::CommandBufferBeginInfo {
        flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT_BIT,
        ..Default::default()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandBufferState {
    Initial,
    Recording,
    Executable,
    Pending,
}

impl Default for CommandBufferState {
    fn default() -> Self {
        CommandBufferState::Initial
    }
}

#[derive(Debug)]
struct XferState {
    // TODO: Can this just be a global instead of being duplicated
    // everywhere?
    dt: Arc<vkl::DeviceTable>,
    gfx_queue: vk::Queue,
    pre_barriers: Vec<vk::ImageMemoryBarrier>,
    post_barriers: Vec<vk::ImageMemoryBarrier>,
    xfer_fence: vk::Fence,
    // secondary; contains only vkCmdCopyImage
    l2_cmds: vk::CommandBuffer,
    // primary; contains vkCmdPipelineBarrier + copy_cmds
    l1_cmds: vk::CommandBuffer,
    rec_state: CommandBufferState,
    size: usize,
    submission_id: u32,
}

impl XferState {
    unsafe fn new(res: &mut InitResources, gfx_qf: u32, len: usize) ->
        Vec<Self>
    {
        let objs = &mut res.objs;
        let mut states = Vec::with_capacity(len);

        let gfx_queue = objs.device.get_queue(gfx_qf, 0);

        let create_info = vk::CommandPoolCreateInfo {
            flags: vk::CommandPoolCreateFlags::TRANSIENT_BIT
                | vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER_BIT,
            queue_family_index: gfx_qf,
            ..Default::default()
        };
        let cmd_pool = objs.create_command_pool(&create_info);

        let l1_alloc_info = vk::CommandBufferAllocateInfo {
            command_pool: cmd_pool,
            command_buffer_count: len as _,
            ..Default::default()
        };
        let mut l1_cbs = vec![vk::CommandBuffer::default(); len];
        objs.alloc_command_buffers(&l1_alloc_info, &mut l1_cbs[..]);

        let l2_alloc_info = vk::CommandBufferAllocateInfo {
            level: vk::CommandBufferLevel::SECONDARY,
            ..l1_alloc_info
        };
        let mut l2_cbs = vec![vk::CommandBuffer::default(); len];
        objs.alloc_command_buffers(&l2_alloc_info, &mut l2_cbs[..]);

        for (&l1, &l2) in l1_cbs.iter().zip(l2_cbs.iter()) {
            states.push(XferState {
                dt: Arc::clone(&objs.device.table),
                gfx_queue,
                pre_barriers: Default::default(),
                post_barriers: Default::default(),
                xfer_fence: objs.create_fence(true),
                l1_cmds: l1,
                l2_cmds: l2,
                rec_state: Default::default(),
                submission_id: Default::default(),
                size: 0,
            });
        }

        states
    }

    unsafe fn _ensure_recording(&mut self) {
        if self.rec_state == CommandBufferState::Recording { return; }
        assert_eq!(self.rec_state, CommandBufferState::Initial);
        self.rec_state = CommandBufferState::Recording;

        let cmds = self.l2_cmds;
        let inheritance_info = Default::default();
        let begin_info = vk::CommandBufferBeginInfo {
            p_inheritance_info: &inheritance_info as _,
            ..begin_one_time()
        };
        self.dt.begin_command_buffer(cmds, &begin_info as _);
    }

    fn _reset(&mut self, submission_id: u32) {
        self.pre_barriers.clear();
        self.post_barriers.clear();
        // N.B. The command buffer probably isn't actually in the initial
        // state here since it gets reset implicitly.
        self.rec_state = CommandBufferState::Initial;
        self.size = 0;
        self.submission_id = submission_id;
    }

    unsafe fn emit_image_copy(&mut self, image: &ImageState) {
        self._ensure_recording();

        let alloc = &image.staging_alloc.as_ref().unwrap().info();

        // Emit pre-barrier
        let barrier = vk::ImageMemoryBarrier {
            dst_access_mask: vk::AccessFlags::TRANSFER_WRITE_BIT,
            old_layout: vk::ImageLayout::UNDEFINED,
            new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            image: image.inner,
            subresource_range: base_image_range(),
            ..Default::default()
        };
        self.pre_barriers.push(barrier);

        // Emit copy
        let extent = image.extent;
        let extent = vk::Extent3D::new(extent.width, extent.height, 1);
        let regions = [vk::BufferImageCopy {
            buffer_offset: alloc.offset,
            buffer_row_length: extent.width,
            buffer_image_height: extent.height,
            image_subresource: vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR_BIT,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            },
            image_offset: vk::Offset3D::new(0, 0, 0),
            image_extent: extent,
        }];
        self.dt.cmd_copy_buffer_to_image(
            self.l2_cmds,                           // commandBuffer,
            alloc.buffer,                           // srcBuffer,
            image.inner,                            // dstImage,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,  // dstImageLayout,
            regions.len() as _,                     // regionCount,
            regions.as_ptr(),                       // pRegions
        );
        self.size += alloc.size as usize;

        // Emit post-barrier
        let barrier = vk::ImageMemoryBarrier {
            src_access_mask: vk::AccessFlags::TRANSFER_WRITE_BIT,
            dst_access_mask: vk::AccessFlags::SHADER_READ_BIT,
            old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            image: image.inner,
            subresource_range: base_image_range(),
            ..Default::default()
        };
        self.post_barriers.push(barrier);
    }

    unsafe fn _record_xfer_cmds(&mut self) {
        let copy_cmds = self.l2_cmds;
        let cmds = self.l1_cmds;
        self.dt.begin_command_buffer(cmds, &begin_one_time() as _);
        self.dt.cmd_pipeline_barrier(
            cmds,                                       // commandBuffer
            vk::PipelineStageFlags::HOST_BIT,           // srcStageMask
            vk::PipelineStageFlags::TRANSFER_BIT,       // dstStageMask
            Default::default(),                 // dependencyFlags
            0,                                  // memoryBarrierCount
            ptr::null(),                        // pMemoryBarriers
            0,                                  // bufferMemoryBarrierCount
            ptr::null(),                        // pBufferMemoryBarriers
            self.pre_barriers.len() as _,       // imageMemoryBarrierCount
            self.pre_barriers.as_ptr(),         // pImageMemoryBarriers
        );
        self.dt.cmd_execute_commands(cmds, 1, &copy_cmds as _);
        self.dt.cmd_pipeline_barrier(
            cmds,                                       // commandBuffer
            vk::PipelineStageFlags::TRANSFER_BIT,       // srcStageMask
            vk::PipelineStageFlags::ALL_GRAPHICS_BIT,   // dstStageMask
            Default::default(),                 // dependencyFlags
            0,                                  // memoryBarrierCount
            ptr::null(),                        // pMemoryBarriers
            0,                                  // bufferMemoryBarrierCount
            ptr::null(),                        // pBufferMemoryBarriers
            self.post_barriers.len() as _,      // imageMemoryBarrierCount
            self.post_barriers.as_ptr(),        // pImageMemoryBarriers
        );
        self.dt.end_command_buffer(cmds).check().unwrap();
    }

    unsafe fn _end_recording(&mut self) {
        if self.rec_state == CommandBufferState::Initial { return; }
        assert_eq!(self.rec_state, CommandBufferState::Recording);
        self.dt.end_command_buffer(self.l2_cmds).check().unwrap();
        self._record_xfer_cmds();
        self.rec_state = CommandBufferState::Executable;
    }

    unsafe fn _submit(&mut self) {
        assert_eq!(self.rec_state, CommandBufferState::Executable);

        let fence = self.xfer_fence;
        self.dt.reset_fences(1, &fence as _).check().unwrap();

        let cmds = std::slice::from_ref(&self.l1_cmds);
        let submit_info = vk::SubmitInfo {
            command_buffer_count: cmds.len() as _,
            p_command_buffers: cmds.as_ptr(),
            ..Default::default()
        };
        self.dt.queue_submit(
            self.gfx_queue,
            1,
            &submit_info as _,
            fence,
        ).check().unwrap();
        self.rec_state = CommandBufferState::Pending;
    }

    unsafe fn flush(&mut self, submission_id: u32) {
        match self.rec_state {
            CommandBufferState::Initial => return,
            CommandBufferState::Recording => self.start_xfer(),
            _ => {},
        }
        assert_eq!(self.rec_state, CommandBufferState::Pending);
        let fence = self.xfer_fence;
        self.dt.wait_for_fences(1, &fence as _, vk::TRUE, u64::max_value())
            .check().unwrap();
        self._reset(submission_id);
    }

    unsafe fn start_xfer(&mut self) {
        self._end_recording();
        self._submit();
    }
}
