use std::sync::Arc;

use derivative::Derivative;
use log::trace;
use more_asserts::assert_lt;
use parking_lot::Mutex;

use crate::*;

/// Hierarchical queue capability classes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum QueueType {
    /// Supports graphics, compute, transfer, and present operations.
    Graphics,
    /// Supports compute and transfer operations.
    Compute,
    /// Supports transfer operations only.
    Xfer,
}

#[derive(Debug)]
pub struct QueueFamily<'dev> {
    device: &'dev Arc<Device>,
    index: u32,
}

#[derive(Debug)]
pub struct Queue {
    device: Arc<Device>,
    inner: vk::Queue,
    family: u32,
    mutex: Mutex<()>,
    name: Option<String>,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct WaitInfo<'a> {
    #[derivative(Debug(format_with = "write_named::<SemaphoreInner>"))]
    pub semaphore: &'a mut SemaphoreInner,
    pub value: u64,
    pub stages: vk::PipelineStageFlags,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct SignalInfo<'a> {
    #[derivative(Debug(format_with = "write_named::<SemaphoreInner>"))]
    pub semaphore: &'a mut SemaphoreInner,
    pub value: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SubmitInfo<'a> {
    pub wait_sems: &'a [WaitInfo<'a>],
    pub sig_sems: &'a [SignalInfo<'a>],
    pub cmds: &'a [vk::CommandBuffer],
}

impl<'dev> QueueFamily<'dev> {
    // The encapsulation here is kind of bad...
    #[inline]
    pub fn new(device: &'dev Arc<Device>, index: u32) -> QueueFamily<'dev> {
        assert_lt!(index as usize, device.queue_families.len());
        QueueFamily { device, index }
    }

    #[inline]
    pub fn device(&self) -> &'dev Arc<Device> {
        self.device
    }

    #[inline]
    pub fn index(&self) -> u32 {
        self.index
    }

    #[inline]
    pub fn properties(&self) -> &'dev vk::QueueFamilyProperties {
        &self.device.queue_families[self.index as usize]
    }

    #[inline]
    pub fn flags(&self) -> vk::QueueFlags {
        self.properties().queue_flags
    }

    pub fn ty(&self) -> QueueType {
        let flags = self.flags();
        if flags.intersects(vk::QueueFlags::GRAPHICS_BIT) {
            assert!(flags.intersects(vk::QueueFlags::COMPUTE_BIT));
            QueueType::Graphics
        } else if flags.intersects(vk::QueueFlags::COMPUTE_BIT) {
            QueueType::Compute
        } else if flags.intersects(vk::QueueFlags::TRANSFER_BIT) {
            QueueType::Xfer
        } else {
            unreachable!();
        }
    }

    #[inline]
    pub fn supports_graphics(&self) -> bool {
        self.ty().supports(QueueType::Graphics)
    }

    #[inline]
    pub fn supports_xfer(&self) -> bool {
        self.ty().supports(QueueType::Xfer)
    }
}

impl Queue {
    #[inline]
    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    #[inline]
    pub fn inner(&self) -> vk::Queue {
        self.inner
    }

    #[inline]
    pub fn family(&self) -> QueueFamily<'_> {
        self.device.queue_family(self.family)
    }

    #[inline]
    pub fn flags(&self) -> vk::QueueFlags {
        self.family().flags()
    }

    #[inline]
    pub fn ty(&self) -> QueueType {
        self.family().ty()
    }

    // TODO: Verify that submitted commands are executable by this type
    // of queue.
    pub unsafe fn submit(&self, submissions: &[SubmitInfo<'_>]) {
        trace!(
            "Queue::submit(self: {:?}, submissions: {:?}",
            fmt_named(self),
            submissions,
        );

        let _lock = self.mutex.lock();

        const MAX_SEMS: usize = 16;
        const MAX_SUBMITS: usize = 8;

        type VecSem<T> = SmallVec<T, MAX_SEMS>;
        type VecSubmit<T> = SmallVec<T, MAX_SUBMITS>;

        let wait_count: usize = submissions
            .iter()
            .map(|submit| submit.wait_sems.len())
            .sum();
        let sig_count: usize = submissions.iter().map(|submit| submit.sig_sems.len()).sum();

        let mut wait_sems = VecSem::with_capacity(wait_count);
        let mut wait_values = VecSem::with_capacity(wait_count);
        let mut wait_stages = VecSem::with_capacity(wait_count);
        let mut sig_sems = VecSem::with_capacity(sig_count);
        let mut sig_values = VecSem::with_capacity(sig_count);
        let mut timelines = VecSubmit::with_capacity(submissions.len());
        let mut infos = VecSubmit::with_capacity(submissions.len());
        for info in submissions.iter() {
            let wait_offset = wait_sems.len();
            for wait in info.wait_sems.iter() {
                wait_sems.push(wait.semaphore.raw());
                wait_values.push(wait.value);
                wait_stages.push(wait.stages);
            }

            let sig_offset = sig_sems.len();
            for sig in info.sig_sems.iter() {
                sig_sems.push(sig.semaphore.raw());
                sig_values.push(sig.value);
            }

            let wait_values = &wait_values[wait_offset..];
            let sig_values = &sig_values[sig_offset..];
            let timeline_info = vk::TimelineSemaphoreSubmitInfo {
                wait_semaphore_value_count: wait_values.len() as _,
                p_wait_semaphore_values: wait_values.as_ptr(),
                signal_semaphore_value_count: sig_values.len() as _,
                p_signal_semaphore_values: sig_values.as_ptr(),
                ..Default::default()
            };
            timelines.push(timeline_info);

            let wait_sems = &wait_sems[wait_offset..];
            let wait_stages = &wait_stages[wait_offset..];
            let sig_sems = &sig_sems[sig_offset..];
            let info = vk::SubmitInfo {
                p_next: timelines.last().unwrap() as *const _ as _,
                wait_semaphore_count: wait_sems.len() as _,
                p_wait_semaphores: wait_sems.as_ptr(),
                p_wait_dst_stage_mask: wait_stages.as_ptr(),
                command_buffer_count: info.cmds.len() as _,
                p_command_buffers: info.cmds.as_ptr(),
                signal_semaphore_count: sig_sems.len() as _,
                p_signal_semaphores: sig_sems.as_ptr(),
                ..Default::default()
            };
            infos.push(info);
        }

        self.device
            .table
            .queue_submit(self.inner, infos.len() as _, infos.as_ptr(), vk::null())
            .check()
            .unwrap();
    }

    pub unsafe fn present(
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
            fmt_named(self),
            DebugIter::new(wait_sems.iter().map(|sem| fmt_named(&**sem))),
            fmt_named(swapchain),
            image,
        );

        let _lock = self.mutex.lock();
        let wait_sems: SmallVec<_, 8> = wait_sems.iter().map(|sem| sem.raw()).collect();
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
        self.device
            .table
            .queue_present_khr(self.inner, &present_info)
    }

    pub(super) unsafe fn get_device_queues(device: &Arc<Device>) -> Vec<Vec<Arc<Queue>>> {
        // TODO: Ughhh... queues are actually hard
        let mut inner = vk::null();
        device.table().get_device_queue(0, 0, &mut inner);

        let mut gfx_queue = Queue {
            device: Arc::clone(device),
            inner,
            family: 0,
            mutex: Mutex::new(()),
            name: None,
        };
        set_name!(gfx_queue);

        vec![vec![Arc::new(gfx_queue)]]
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        let name: String = name.into();
        self.name = Some(name.clone());
        unsafe {
            self.device().set_name(self.inner(), name);
        }
    }
}

impl Named for Queue {
    fn name(&self) -> Option<&str> {
        Some(&self.name.as_ref()?)
    }
}

impl QueueType {
    #[inline]
    pub fn supports(self, other: Self) -> bool {
        self <= other
    }
}
