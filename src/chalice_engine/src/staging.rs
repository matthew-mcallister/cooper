use std::sync::Arc;

use bitflags::bitflags;
use device::{CmdBuffer, Device, Image, MemoryRegion, Queue};

/// Handles uploading data from the host to the device. Both the
/// discrete and UMA cases are equally handled.
#[derive(Debug)]
pub struct StagingBuffer {
    graphics_queue: Arc<Queue>,
    transfer_queue: Arc<Queue>,
    buffer: device::DeviceBuffer,
    offset: usize,
    semaphore: device::TimelineSemaphore,
    pending_transfer: u64,
}

bitflags! {
    #[derive(Default)]
    pub struct StageFlags: u32 {
        const NO_TRANSITION = 0x01;
    }
}

impl StagingBuffer {
    pub fn new(
        graphics_queue: Arc<Queue>,
        transfer_queue: Arc<Queue>,
        size: vk::DeviceSize,
    ) -> Self {
        let device = graphics_queue.device();
        let semaphore = device::TimelineSemaphore::new(Arc::clone(&device), 0);
        let buffer = device::DeviceBuffer::new(
            Arc::clone(device),
            size,
            device::BufferUsage::TRANSFER_SRC,
            device::MemoryMapping::Mapped,
            device::Lifetime::Static,
        );
        Self {
            graphics_queue,
            transfer_queue,
            buffer,
            offset: 0,
            semaphore,
            pending_transfer: 0,
        }
    }

    pub fn device(&self) -> &Arc<Device> {
        self.buffer.device()
    }

    pub fn pending(&self) -> bool {
        self.semaphore.get_value() != self.pending_transfer
    }

    fn stage_data(&mut self, src: &[u8]) -> Option<usize> {
        assert!(!self.pending());
        let end = self.offset + src.len();
        if end as vk::DeviceSize > self.buffer.size() {
            return None;
        }
        self.buffer.as_bytes_mut().unwrap()[self.offset..end].copy_from_slice(src);
        let offset = self.offset;
        self.offset += src.len();
        Some(offset)
    }

    pub fn stage_buffer(
        &mut self,
        cmds: &mut CmdBuffer<'_>,
        src: &[u8],
        // TODO: smh... BufferRange holds an immutable reference to the
        // byte array, which we are accessing here mutably...
        // Need a BufferRangeMut struct.
        dest: &mut device::BufferRange<'_>,
        _flags: StageFlags,
    ) -> Option<()> {
        if let Some(bytes) = dest.as_bytes_mut() {
            bytes.copy_from_slice(src);
        } else {
            let offset = self.stage_data(src)?;
            unsafe {
                cmds.copy_buffer(
                    &self.buffer,
                    dest.buffer,
                    &[vk::BufferCopy {
                        src_offset: offset as _,
                        dst_offset: dest.offset,
                        size: src.len() as _,
                    }],
                );
            }
        }
        Some(())
    }

    pub fn stage_image_layers(
        &mut self,
        cmds: &mut CmdBuffer<'_>,
        src: &[u8],
        dest: &Arc<Image>,
        base_layer: u32,
        layer_count: u32,
        flags: StageFlags,
    ) -> Option<()> {
        let offset = self.stage_data(src)?;
        unsafe {
            let subresource_range: vk::ImageSubresourceRange =
                dest.subresource_layers(0, base_layer, layer_count).into();
            if !flags.contains(StageFlags::NO_TRANSITION) {
                cmds.pipeline_barrier(
                    vk::PipelineStageFlags::TOP_OF_PIPE_BIT,
                    vk::PipelineStageFlags::TRANSFER_BIT,
                    Default::default(),
                    &[],
                    &[],
                    &[vk::ImageMemoryBarrier {
                        src_access_mask: vk::AccessFlags::empty(),
                        dst_access_mask: vk::AccessFlags::TRANSFER_WRITE_BIT,
                        old_layout: vk::ImageLayout::UNDEFINED,
                        new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        src_queue_family_index: self.transfer_queue.family().index(),
                        dst_queue_family_index: self.graphics_queue.family().index(),
                        image: dest.inner(),
                        subresource_range,
                        ..Default::default()
                    }],
                );
            }
            cmds.copy_buffer_to_image(
                &self.buffer,
                &dest,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[vk::BufferImageCopy {
                    buffer_offset: offset as _,
                    image_subresource: dest.subresource_layers(0, base_layer, layer_count).into(),
                    image_extent: dest.extent().into(),
                    ..Default::default()
                }],
            );
            if !flags.contains(StageFlags::NO_TRANSITION) {
                cmds.pipeline_barrier(
                    vk::PipelineStageFlags::TRANSFER_BIT,
                    vk::PipelineStageFlags::FRAGMENT_SHADER_BIT,
                    Default::default(),
                    &[],
                    &[],
                    &[vk::ImageMemoryBarrier {
                        src_access_mask: vk::AccessFlags::TRANSFER_WRITE_BIT,
                        dst_access_mask: vk::AccessFlags::SHADER_READ_BIT,
                        old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        src_queue_family_index: self.transfer_queue.family().index(),
                        dst_queue_family_index: self.graphics_queue.family().index(),
                        image: dest.inner(),
                        subresource_range,
                        ..Default::default()
                    }],
                );
            }
        }
        Some(())
    }

    pub fn stage_image(
        &mut self,
        cmds: &mut CmdBuffer<'_>,
        src: &[u8],
        dest: &Arc<Image>,
        flags: StageFlags,
    ) -> Option<()> {
        self.stage_image_layers(cmds, src, dest, 0, 1, flags)
    }

    pub fn submit(&mut self, cmds: CmdBuffer<'_>) -> u64 {
        assert!(!self.pending());
        self.offset = 0;
        self.pending_transfer += 1;
        let cmds = cmds.end();
        unsafe {
            self.transfer_queue.submit(&[device::SubmitInfo {
                wait_sems: &[],
                sig_sems: &[device::SignalInfo {
                    semaphore: self.semaphore.inner_mut(),
                    value: self.pending_transfer,
                }],
                cmds: &[cmds],
            }]);
        }
        self.pending_transfer
    }

    #[inline]
    pub fn semaphore_mut(&mut self) -> &mut device::TimelineSemaphore {
        &mut self.semaphore
    }

    pub fn wait(&self, timeout: u64) -> device::WaitResult {
        self.semaphore.wait(self.pending_transfer, timeout)
    }
}
