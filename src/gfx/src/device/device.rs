use std::ffi::CString;
use std::ptr;
use std::sync::{Arc, Mutex};

use derivative::Derivative;
use log::trace;
use prelude::*;

use crate::*;

#[derive(Derivative)]
#[derivative(Debug)]
crate struct Device {
    #[derivative(Debug = "ignore")]
    crate table: Arc<vkl::DeviceTable>,
    crate instance: Arc<Instance>,
    crate app_info: Arc<AppInfo>,
    crate pdev: vk::PhysicalDevice,
    crate props: vk::PhysicalDeviceProperties,
    crate queue_families: Vec<vk::QueueFamilyProperties>,
    crate mem_props: vk::PhysicalDeviceMemoryProperties,
    crate features: vk::PhysicalDeviceFeatures,
}

impl Drop for Device {
    fn drop(&mut self) {
        let dt = &*self.table;
        unsafe {
            dt.destroy_device(ptr::null());
        }
    }
}

/// Hierarchical queue capability classes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
crate enum QueueType {
    /// Supports graphics, compute, transfer, and present operations.
    Graphics,
    /// Supports compute and transfer operations.
    Compute,
    /// Supports transfer operations only.
    Transfer,
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
    crate wait_sems: &'a [&'a Semaphore],
    crate wait_stages: &'a [vk::PipelineStageFlags],
    crate sig_sems: &'a [&'a Semaphore],
    crate cmds: &'a [vk::CommandBuffer],
}

impl<'dev> QueueFamily<'dev> {
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
            QueueType::Transfer
        } else {
            unreachable!();
        }
    }

    crate fn supports_graphics(&self) -> bool {
        self.ty().supports(QueueType::Graphics)
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
            "submitting commands: queue: {:?}, submissions: {:?}, fence: {:?}",
            self, submissions, fence,
        );

        let _lock = self.mutex.lock();

        let mut sems = Vec::with_capacity(submissions.len());
        let submissions: Vec<_> = submissions.iter().map(|info| {
            let wait_sems: Vec<_> = info.wait_sems.iter()
                .map(|sem| sem.inner()).collect();
            let sig_sems: Vec<_> = info.sig_sems.iter()
                .map(|sem| sem.inner()).collect();
            let info = vk::SubmitInfo {
                wait_semaphore_count: wait_sems.len() as _,
                p_wait_semaphores: wait_sems.as_ptr(),
                p_wait_dst_stage_mask: info.wait_stages.as_ptr(),
                command_buffer_count: info.cmds.len() as _,
                p_command_buffers: info.cmds.as_ptr(),
                signal_semaphore_count: sig_sems.len() as _,
                p_signal_semaphores: sig_sems.as_ptr(),
                ..Default::default()
            };
            sems.push((wait_sems, sig_sems));
            info
        }).collect();

        self.device.table.queue_submit(
            self.inner,
            submissions.len() as _,
            submissions.as_ptr(),
            try_opt!(fence?.inner()).unwrap_or(vk::null()),
        ).check().unwrap();
    }

    crate unsafe fn present(
        &self,
        wait_sems: &[&Semaphore],
        swapchain: &mut Swapchain,
        image: u32,
    ) -> vk::Result {
        trace!(
            concat!(
                "presenting to queue: queue: {:?}, wait_sems: {:?}, ",
                "swapchain: {:?}, image: {}",
            ),
            self, wait_sems, swapchain, image,
        );

        let _lock = self.mutex.lock();
        let wait_sems: Vec<_> = wait_sems.iter().map(|sem| sem.inner())
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
}

impl QueueType {
    crate fn supports(self, other: Self) -> bool {
        self <= other
    }
}

impl Device {
    crate unsafe fn new(instance: Arc<Instance>, pdev: vk::PhysicalDevice) ->
        Result<(Arc<Self>, Vec<Vec<Arc<Queue>>>), AnyError>
    {
        let it = &instance.table;
        let app_info = Arc::clone(&instance.app_info);

        // TODO: check that extensions are actually supported
        let exts = [
            vk::KHR_SWAPCHAIN_EXTENSION_NAME,
        ];

        let features = vk::PhysicalDeviceFeatures {
            sampler_anisotropy: vk::TRUE,
            ..Default::default()
        };

        let queue_infos = [vk::DeviceQueueCreateInfo {
            queue_family_index: 0,
            queue_count: 1,
            p_queue_priorities: &1f32,
            ..Default::default()
        }];

        let create_info = vk::DeviceCreateInfo {
            queue_create_info_count: queue_infos.len() as _,
            p_queue_create_infos: queue_infos.as_ptr(),
            enabled_extension_count: exts.len() as _,
            pp_enabled_extension_names: exts.as_ptr(),
            p_enabled_features: &features,
            ..Default::default()
        };
        let mut dev = vk::null();
        it.create_device(pdev, &create_info, ptr::null(), &mut dev)
            .check()?;

        let get_device_proc_addr = std::mem::transmute({
            it.get_instance_proc_addr(c_str!("vkGetDeviceProcAddr"))
        });
        let table =
            Arc::new(vkl::DeviceTable::load(dev, get_device_proc_addr));

        let props = instance.get_properties(pdev);
        let queue_families = instance.get_queue_family_properties(pdev);
        let mut mem_props = Default::default();
        it.get_physical_device_memory_properties(pdev, &mut mem_props);

        let device = Arc::new(Device {
            table,
            instance,
            app_info,
            pdev,
            props,
            queue_families,
            mem_props,
            features,
        });

        let queues = device.get_queues();

        Ok((device, queues))
    }

    unsafe fn get_queues(self: &Arc<Self>) -> Vec<Vec<Arc<Queue>>> {
        let mut inner = vk::null();
        self.table.get_device_queue(0, 0, &mut inner);

        let queue = Arc::new(Queue {
            device: Arc::clone(self),
            inner,
            family: 0,
            mutex: Mutex::new(()),
        });

        vec![vec![queue]]
    }

    crate fn properties(&self) -> &vk::PhysicalDeviceProperties {
        &self.props
    }

    crate fn queue_family<'dev>(
        self: &'dev Arc<Self>,
        index: u32,
    ) -> QueueFamily<'dev> {
        QueueFamily {
            device: self,
            index,
        }
    }

    crate fn limits(&self) -> &vk::PhysicalDeviceLimits {
        &self.properties().limits
    }

    crate fn features(&self) -> &vk::PhysicalDeviceFeatures {
        &self.features
    }

    crate fn set_name(
        &self,
        obj: &impl Debuggable,
        name: &(impl AsRef<str> + ?Sized),
    ) {
        if self.app_info.debug {
            let name = CString::new(name.as_ref()).unwrap();
            unsafe { set_name(&self.table, obj, name.as_ptr()); }
        }
    }

    crate unsafe fn create_swapchain(self: &Arc<Self>, surface: &Arc<Surface>)
        -> Result<Swapchain, AnyError>
    {
        Ok(Swapchain::new(Arc::clone(surface), Arc::clone(self))?)
    }

    crate fn wait_idle(&self) {
        unsafe { self.table.device_wait_idle(); }
    }
}
