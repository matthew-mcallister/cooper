use std::sync::Arc;

use log::trace;
use more_asserts::assert_lt;
use parking_lot::Mutex;

use crate::*;

/// Hierarchical queue capability classes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
crate enum QueueType {
    /// Supports graphics, compute, transfer, and present operations.
    Graphics,
    /// Supports compute and transfer operations.
    Compute,
    /// Supports transfer operations only.
    Xfer,
}

#[derive(Debug)]
crate struct QueueFamily<'dev> {
    device: &'dev Arc<Device>,
    index: u32,
}

#[derive(Debug)]
crate struct Queue {
    device: Arc<Device>,
    inner: vk::Queue,
    family: u32,
    mutex: Mutex<()>,
}

#[derive(Clone, Copy, Debug, Default)]
crate struct SubmitInfo<'a> {
    crate wait_sems: &'a [SemaphoreInner<'a>],
    crate wait_values: &'a [u64],
    crate wait_stages: &'a [vk::PipelineStageFlags],
    crate sig_sems: &'a [SemaphoreInner<'a>],
    crate sig_values: &'a [u64],
    crate cmds: &'a [vk::CommandBuffer],
}

impl<'dev> QueueFamily<'dev> {
    // The encapsulation here is kind of bad...
    crate fn new(
        device: &'dev Arc<Device>,
        index: u32,
    ) -> QueueFamily<'dev> {
        assert_lt!(index as usize, device.queue_families.len());
        QueueFamily {
            device,
            index,
        }
    }

    crate fn device(&self) -> &'dev Arc<Device> {
        self.device
    }

    crate fn index(&self) -> u32 {
        self.index
    }

    crate fn properties(&self) -> &'dev vk::QueueFamilyProperties {
        &self.device.queue_families[self.index as usize]
    }

    crate fn flags(&self) -> vk::QueueFlags {
        self.properties().queue_flags
    }

    crate fn ty(&self) -> QueueType {
        let flags = self.flags();
        if flags.intersects(vk::QueueFlags::GRAPHICS_BIT) {
            debug_assert!(flags.intersects(vk::QueueFlags::COMPUTE_BIT));
            QueueType::Graphics
        } else if flags.intersects(vk::QueueFlags::COMPUTE_BIT) {
            QueueType::Compute
        } else if flags.intersects(vk::QueueFlags::TRANSFER_BIT) {
            QueueType::Xfer
        } else {
            unreachable!();
        }
    }

    crate fn supports_graphics(&self) -> bool {
        self.ty().supports(QueueType::Graphics)
    }

    crate fn supports_xfer(&self) -> bool {
        self.ty().supports(QueueType::Xfer)
    }
}

impl Queue {
    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    crate fn inner(&self) -> vk::Queue {
        self.inner
    }

    crate fn family(&self) -> QueueFamily<'_> {
        self.device.queue_family(self.family)
    }

    crate fn flags(&self) -> vk::QueueFlags {
        self.family().flags()
    }

    crate fn ty(&self) -> QueueType {
        self.family().ty()
    }

    // TODO: Verify that submitted commands are executable by this type
    // of queue.
    crate unsafe fn submit(
        &self,
        submissions: &[SubmitInfo],
        fence: Option<&mut Fence>,
    ) {
        trace!(
            "Queue::submit(self: {:?}, submissions: {:?}, fence: {:?})",
            self, submissions, fence,
        );

        let _lock = self.mutex.lock();

        let mut timelines: SmallVec<_, 8> =
            SmallVec::with_capacity(submissions.len());
        let mut infos: SmallVec<_, 8> =
            SmallVec::with_capacity(submissions.len());
        for info in submissions.iter() {
            // The lengths don't have to be equal but it doens't hurt to
            // enforce that they are.
            assert_eq!(info.wait_sems.len(), info.wait_values.len());
            assert_eq!(info.sig_sems.len(), info.sig_values.len());
            let wait_sems = SemaphoreInner::slice_as_raw(info.wait_sems);
            let sig_sems = SemaphoreInner::slice_as_raw(info.sig_sems);
            let timeline_info = vk::TimelineSemaphoreSubmitInfo {
                wait_semaphore_value_count: info.wait_values.len() as _,
                p_wait_semaphore_values: info.wait_values.as_ptr(),
                signal_semaphore_value_count: info.sig_values.len() as _,
                p_signal_semaphore_values: info.sig_values.as_ptr(),
                ..Default::default()
            };
            timelines.push(timeline_info);

            let info = vk::SubmitInfo {
                p_next: timelines.last().unwrap() as *const _ as _,
                wait_semaphore_count: wait_sems.len() as _,
                p_wait_semaphores: wait_sems.as_ptr(),
                p_wait_dst_stage_mask: info.wait_stages.as_ptr(),
                command_buffer_count: info.cmds.len() as _,
                p_command_buffers: info.cmds.as_ptr(),
                signal_semaphore_count: sig_sems.len() as _,
                p_signal_semaphores: sig_sems.as_ptr(),
                ..Default::default()
            };
            infos.push(info);
        }

        self.device.table.queue_submit(
            self.inner,
            infos.len() as _,
            infos.as_ptr(),
            try_opt!(fence?.inner()).unwrap_or(vk::null()),
        ).check().unwrap();
    }

    crate unsafe fn present(
        &self,
        wait_sems: &[&mut BinarySemaphore],
        swapchain: &mut Swapchain,
        image: u32,
    ) -> vk::Result {
        trace!(
            concat!(
                "Queue::present(self: {:?}, wait_sems: {:?}, ",
                "swapchain: {:?}, image: {})",
            ),
            self, wait_sems, swapchain, image,
        );

        let _lock = self.mutex.lock();
        let wait_sems: SmallVec<_, 8> = wait_sems.iter().map(|sem| sem.raw())
            .collect();
        let swapchains = [swapchain.inner];
        let images = [image];
        let present_info = vk::PresentInfoKHR {
            wait_semaphore_count: wait_sems.len() as _,
            p_wait_semaphores: wait_sems.as_ptr(),
            swapchain_count: swapchains.len() as _,
            p_swapchains: swapchains.as_ptr(),
            p_image_indices: images.as_ptr(),
            ..Default::default()
        };
        self.device.table.queue_present_khr(self.inner, &present_info)
    }

    pub(super) unsafe fn get_device_queues(device: &Arc<Device>) ->
        Vec<Vec<Arc<Queue>>>
    {
        let mut inner = vk::null();
        device.table().get_device_queue(0, 0, &mut inner);

        let queue = Arc::new(Queue {
            device: Arc::clone(device),
            inner,
            family: 0,
            mutex: Mutex::new(()),
        });

        vec![vec![queue]]
    }
}

impl QueueType {
    // TODO: This implementation seems overly clever. In particular,
    // graphics queues aren't *guaranteed* to support compute, though
    // they always do in practice.
    crate fn supports(self, other: Self) -> bool {
        self <= other
    }
}
