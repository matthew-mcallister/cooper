use std::ffi::CString;
use std::ptr;
use std::sync::{Arc, Mutex};

use prelude::*;

use crate::*;

#[derive(Debug)]
crate struct Device {
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
        submissions: &[vk::SubmitInfo],
        fence: vk::Fence,
    ) {
        let _lock = self.mutex.lock();
        self.device.table.queue_submit(
            self.inner,
            submissions.len() as _,
            submissions.as_ptr(),
            fence,
        ).check().unwrap();
    }

    crate unsafe fn present(&self, present_info: &vk::PresentInfoKHR) ->
        vk::Result
    {
        let _lock = self.mutex.lock();
        self.device.table.queue_present_khr(self.inner, present_info)
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

    crate unsafe fn set_debug_name<T, A>(&self, obj: T, name: A)
    where
        T: DebugUtils,
        A: AsRef<str>,
    {
        if self.app_info.debug {
            let name = CString::new(name.as_ref()).unwrap();
            set_debug_name(&self.table, obj, name.as_ptr());
        }
    }

    crate unsafe fn create_fence(&self, signaled: bool) -> vk::Fence {
        let mut create_info = vk::FenceCreateInfo::default();
        if signaled {
            create_info.flags |= vk::FenceCreateFlags::SIGNALED_BIT;
        }
        let mut obj = vk::null();
        self.table.create_fence(&create_info, ptr::null(), &mut obj)
            .check().unwrap();
        obj
    }

    crate unsafe fn create_semaphore(&self) -> vk::Semaphore {
        let create_info = Default::default();
        let mut obj = vk::null();
        self.table.create_semaphore(&create_info, ptr::null(), &mut obj)
            .check().unwrap();
        obj
    }

    crate unsafe fn create_swapchain(self: &Arc<Self>, surface: &Arc<Surface>)
        -> Result<Arc<Swapchain>, AnyError>
    {
        Ok(Arc::new(Swapchain::new(Arc::clone(surface), Arc::clone(self))?))
    }
}
