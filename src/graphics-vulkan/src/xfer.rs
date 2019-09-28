use std::num::NonZeroU32;
use std::ptr;
use std::sync::Arc;

use crate::*;

/// Serial number corresponding to a transfer batch.
pub type XferBatchSerial = NonZeroU32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CmdBufferState {
    Initial,
    Recording,
    Executable,
    Pending,
}

impl Default for CmdBufferState {
    fn default() -> Self {
        CmdBufferState::Initial
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XferState {
    Clean,
    Dirty,
    Pending,
}

#[derive(Debug)]
struct XferCmdBuffer {
    dt: Arc<vkl::DeviceTable>,
    queue: Arc<Queue>,
    state: CmdBufferState,
    fence: vk::Fence,
    /// primary; contains vkCmdPipelineBarrier + img_l2
    img_l1: vk::CommandBuffer,
    /// secondary; contains only vkCmdCopyImage
    img_l2: vk::CommandBuffer,
    img_pre_barriers: Vec<vk::ImageMemoryBarrier>,
    img_post_barriers: Vec<vk::ImageMemoryBarrier>,
    // TODO:
    //buf_cmds: vk::CommandBuffer,
}

impl Drop for XferCmdBuffer {
    fn drop(&mut self) {
        unsafe {
            self.dt.destroy_fence(self.fence, ptr::null());
        }
    }
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

#[inline]
fn begin_one_time() -> vk::CommandBufferBeginInfo {
    vk::CommandBufferBeginInfo {
        flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT_BIT,
        ..Default::default()
    }
}

impl XferCmdBuffer {
    unsafe fn new(
        queue: Arc<Queue>,
        cmd_pool: vk::CommandPool,
        len: usize,
    ) -> Vec<Self> {
        let dt = Arc::clone(&queue.device.table);

        let l1_alloc_info = vk::CommandBufferAllocateInfo {
            command_pool: cmd_pool,
            command_buffer_count: len as _,
            ..Default::default()
        };
        let mut l1_cbs = vec![vk::CommandBuffer::default(); len];
        dt.allocate_command_buffers(&l1_alloc_info as _, l1_cbs.as_mut_ptr())
            .check().unwrap();

        let l2_alloc_info = vk::CommandBufferAllocateInfo {
            level: vk::CommandBufferLevel::SECONDARY,
            ..l1_alloc_info
        };
        let mut l2_cbs = vec![vk::CommandBuffer::default(); len];
        dt.allocate_command_buffers(&l2_alloc_info as _, l2_cbs.as_mut_ptr())
            .check().unwrap();

        l1_cbs.into_iter().zip(l2_cbs.into_iter())
            .map(|(img_l1, img_l2)| XferCmdBuffer {
                dt: Arc::clone(&dt),
                queue: Arc::clone(&queue),
                state: Default::default(),
                fence: queue.device.create_fence(true),
                img_pre_barriers: Default::default(),
                img_post_barriers: Default::default(),
                img_l1,
                img_l2,
            })
            .collect()
    }

    unsafe fn _ensure_recording(&mut self) {
        if self.state == CmdBufferState::Recording { return; }
        assert_eq!(self.state, CmdBufferState::Initial);
        self.state = CmdBufferState::Recording;

        let cmds = self.img_l2;
        let inheritance_info = Default::default();
        let begin_info = vk::CommandBufferBeginInfo {
            p_inheritance_info: &inheritance_info as _,
            ..begin_one_time()
        };
        self.dt.begin_command_buffer(cmds, &begin_info as _);
    }

    fn _reset(&mut self) {
        self.img_pre_barriers.clear();
        self.img_post_barriers.clear();
        // N.B. The cmd buf possibly isn't actually in the initial state
        // yet since it is reset implicitly by vkBeginCommandBuffer.
        self.state = CmdBufferState::Initial;
    }

    // TODO: Queue ownership transfer
    unsafe fn emit_image_copy(&mut self, image: &Image, src: &AllocInfo) {
        self._ensure_recording();

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
        self.img_pre_barriers.push(barrier);

        // Emit copy
        let extent = image.extent;
        let extent = vk::Extent3D::new(extent.width, extent.height, 1);
        let regions = [vk::BufferImageCopy {
            buffer_offset: src.offset,
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
            self.img_l2,                            // commandBuffer,
            src.buffer,                             // srcBuffer,
            image.inner,                            // dstImage,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,  // dstImageLayout,
            regions.len() as _,                     // regionCount,
            regions.as_ptr(),                       // pRegions
        );

        // Emit post-barrier
        let barrier = vk::ImageMemoryBarrier {
            src_access_mask: vk::AccessFlags::TRANSFER_WRITE_BIT,
            dst_access_mask: image.dst_access_mask,
            old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            new_layout: image.dst_layout,
            src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            image: image.inner,
            subresource_range: base_image_range(),
            ..Default::default()
        };
        self.img_post_barriers.push(barrier);
    }

    unsafe fn _record_img_l1(&mut self) {
        assert_eq!(self.img_pre_barriers.len(), self.img_post_barriers.len());
        assert!(!self.img_pre_barriers.is_empty());

        let copy_cmds = self.img_l2;
        let cmds = self.img_l1;
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
            self.img_pre_barriers.len() as _,   // imageMemoryBarrierCount
            self.img_pre_barriers.as_ptr(),     // pImageMemoryBarriers
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
            self.img_post_barriers.len() as _,  // imageMemoryBarrierCount
            self.img_post_barriers.as_ptr(),    // pImageMemoryBarriers
        );
        self.dt.end_command_buffer(cmds).check().unwrap();
    }

    unsafe fn _end_recording(&mut self) {
        if self.state == CmdBufferState::Initial { return; }
        assert_eq!(self.state, CmdBufferState::Recording);
        self.dt.end_command_buffer(self.img_l2).check().unwrap();
        self._record_img_l1();
        self.state = CmdBufferState::Executable;
    }

    unsafe fn _submit(&mut self) {
        assert_eq!(self.state, CmdBufferState::Executable);

        let fence = self.fence;
        self.dt.reset_fences(1, &fence as _).check().unwrap();

        let cmds = &[self.img_l1];
        let submit_info = vk::SubmitInfo {
            command_buffer_count: cmds.len() as _,
            p_command_buffers: cmds.as_ptr(),
            ..Default::default()
        };
        self.queue.submit(&[submit_info], fence);
        self.state = CmdBufferState::Pending;
    }

    fn state(&self) -> XferState {
        match self.state {
            CmdBufferState::Initial => XferState::Clean,
            CmdBufferState::Recording => XferState::Dirty,
            CmdBufferState::Executable => unreachable!(),
            CmdBufferState::Pending => XferState::Pending,
        }
    }

    // Submits staged commands, if any.
    unsafe fn submit(&mut self) {
        if self.state == CmdBufferState::Recording {
            self._end_recording();
            self._submit();
        }
    }

    // Updates the current transfer state.
    unsafe fn poll(&mut self) {
        if self.state != CmdBufferState::Pending { return; }
        let status = self.dt.get_fence_status(self.fence);
        if status == vk::Result::SUCCESS {
            self._reset();
        }
    }

    // Waits for any pending transfer commands to complete.
    unsafe fn wait(&mut self) {
        if self.state != CmdBufferState::Pending {
            return;
        }
        let fence = self.fence;
        self.dt.wait_for_fences(1, &fence as _, vk::TRUE, u64::max_value())
            .check().unwrap();
        self._reset();
    }
}

#[derive(Debug)]
pub struct XferBatchState {
    cmds: XferCmdBuffer,
    staging: StagingBuffer,
}

/// This type wraps a single transfer-capable queue, equipping it with
/// staging memory and command buffers, and handling transfer details
/// behind the scenes. It may wrap either a dedicated transfer queue
/// (TODO: not implemented) or a multipurpose queue on either a discrete
/// or unified memory architecture system.
///
/// If multiple transfer queues are available, it may be possible to
/// operate multiple instances of this type in parallel.
#[derive(Debug)]
pub struct XferQueue {
    queue: Arc<Queue>,
    batch_size: usize,
    cmd_pool: vk::CommandPool,
    // Double-buffered so we can copy while transferring
    batches: [XferBatchState; 2],
    serial: XferBatchSerial,
}

impl Drop for XferQueue {
    fn drop(&mut self) {
        unsafe {
            // TODO: It might be worth bringing back scoped Vulkan
            // object pools to avoid writing this kind of destructor.
            // Should mix well with scoped allocators.
            self.queue.device.table
                .destroy_command_pool(self.cmd_pool as _, ptr::null());
        }
    }
}

macro_rules! batches {
    ($self:expr) => {
        &mut $self.batches[$self.idx()]
    }
}

macro_rules! cmds {
    ($self:expr) => {
        &mut batches!($self).cmds
    }
}

macro_rules! staging {
    ($self:expr) => {
        &mut batches!($self).staging
    }
}

impl XferQueue {
    #[inline(always)]
    fn idx(&self) -> usize {
        self.serial.get() as usize % self.batches.len()
    }

    pub unsafe fn new(queue: Arc<Queue>, batch_size: usize) -> Self {
        let queue_flags = queue.family.properties.queue_flags;
        assert!(queue_flags.contains(vk::QueueFlags::TRANSFER_BIT));

        let dt = &queue.device.table;

        let create_info = vk::CommandPoolCreateInfo {
            flags: vk::CommandPoolCreateFlags::TRANSIENT_BIT
                | vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER_BIT,
            queue_family_index: queue.family.index,
            ..Default::default()
        };
        let mut cmd_pool = vk::null();
        dt.create_command_pool
            (&create_info as _, ptr::null(), &mut cmd_pool as _)
            .check().unwrap();

        let mut cmds = XferCmdBuffer::new(Arc::clone(&queue), cmd_pool, 2);

        let device = &queue.device;
        let batches = [
            XferBatchState {
                staging: StagingBuffer::new(Arc::clone(&device), batch_size),
                cmds: cmds.pop().unwrap(),
            },
            XferBatchState {
                staging: StagingBuffer::new(Arc::clone(&device), batch_size),
                cmds: cmds.pop().unwrap(),
            },
        ];

        XferQueue {
            queue,
            batch_size,
            cmd_pool,
            batches,
            serial: NonZeroU32::new_unchecked(1),
        }
    }

    fn ensure_clear(&mut self) {
        if cmds!(self).state() == XferState::Clean {
            staging!(self).clear();
        }
    }

    /// Tries to stage an image for upload. Returns `None` when the
    /// queue isn't ready to accept more data. Otherwise, returns a
    /// slice pointer where the image data can be written.
    pub unsafe fn stage_image(
        &mut self,
        image: &mut Image,
    ) -> Option<*mut [u8]> {
        assert!(image.bound_alloc.is_some());
        self.ensure_clear();

        let size = image.size;
        let alloc = staging!(self).allocate(size)?;
        image.batch_serial = Some(self.serial);

        cmds!(self).emit_image_copy(image, alloc.info());

        let ptr = alloc.info().ptr as *mut u8;
        let slice = std::slice::from_raw_parts_mut(ptr, size);
        Some(slice as _)
    }

    fn next_batch(&mut self) {
        self.serial = NonZeroU32::new(self.serial.get() + 1).unwrap();
    }

    pub unsafe fn submit(&mut self) {
        if self.state() == XferState::Dirty {
            cmds!(self).submit();
            self.next_batch();
        }
    }

    pub fn state(&self) -> XferState {
        self.batches[self.idx()].cmds.state()
    }

    pub unsafe fn poll(&mut self) {
        cmds!(self).poll();
    }

    pub unsafe fn wait(&mut self) {
        cmds!(self).wait();
    }

    /// Waits for all pending transfers to complete.
    pub unsafe fn flush(&mut self) {
        self.submit();
        self.batches[0].cmds.wait();
        self.batches[1].cmds.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let swapchain = vars.swapchain;
        let queue = Arc::clone(&vars.queues[0][0]);
        let dt = &*swapchain.device.table;
        let device = Arc::clone(&swapchain.device);

        let mut image_mem = create_image_mem(device, 0x400_0000);

        let mut images: Vec<_> = (0..64).map(|_| {
            let extent = vk::Extent3D::new(256, 256, 1);
            let format = vk::Format::R8G8B8A8_SRGB;
            let size = (extent.width * extent.height * 4) as _;
            let create_info = vk::ImageCreateInfo {
                image_type: vk::ImageType::_2D,
                format: vk::Format::R8G8B8A8_SRGB,
                extent,
                mip_levels: 1,
                array_layers: 1,
                samples: vk::SampleCountFlags::_1_BIT,
                tiling: vk::ImageTiling::OPTIMAL,
                usage: vk::ImageUsageFlags::TRANSFER_DST_BIT |
                    vk::ImageUsageFlags::SAMPLED_BIT,
                initial_layout: vk::ImageLayout::UNDEFINED,
                ..Default::default()
            };
            let mut inner = vk::null();
            dt.create_image(&create_info as _, ptr::null(), &mut inner as _)
                .check().unwrap();

            let view = vk::null();

            let alloc = image_mem.alloc_image_memory(inner);

            Image {
                inner,
                view,
                extent,
                format,
                dst_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                dst_access_mask: vk::AccessFlags::SHADER_READ_BIT,
                size,
                batch_serial: None,
                bound_alloc: Some(alloc),
            }
        }).collect();

        let mut xfer = XferQueue::new(queue, 0x4_0000);
        for image in images.iter_mut() {
            let slice = xfer.stage_image(image)
                .or_else(|| {
                    xfer.submit();
                    xfer.wait();
                    xfer.stage_image(image)
                })
                .unwrap();
            // Fill with zeroes
            (&mut *slice).iter_mut().for_each(|x| *x = 0);
        }
        xfer.flush();

        for image in images.into_iter() {
            dt.destroy_image(image.inner, ptr::null());
        }
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
