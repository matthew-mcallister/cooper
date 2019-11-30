use std::ffi::CString;
use std::ptr;
use std::sync::{Arc, Mutex};

use prelude::*;

use crate::*;

#[derive(Debug)]
crate struct Device {
    crate instance: Arc<Instance>,
    crate app_info: Arc<AppInfo>,
    crate pdev: vk::PhysicalDevice,
    crate props: Box<vk::PhysicalDeviceProperties>,
    crate mem_props: Box<vk::PhysicalDeviceMemoryProperties>,
    crate table: Arc<vkl::DeviceTable>,
}

#[derive(Debug)]
crate struct QueueFamily {
    index: u32,
    properties: vk::QueueFamilyProperties,
}

impl QueueFamily {
    crate fn index(&self) -> u32 {
        self.index
    }

    crate fn properties(&self) -> &vk::QueueFamilyProperties {
        &self.properties
    }

    crate fn flags(&self) -> vk::QueueFlags {
        self.properties.queue_flags
    }
}

#[derive(Debug)]
crate struct Queue {
    device: Arc<Device>,
    inner: vk::Queue,
    family: Arc<QueueFamily>,
    mutex: Mutex<()>,
}

impl Queue {
    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    crate fn inner(&self) -> vk::Queue {
        self.inner
    }

    crate fn family(&self) -> &Arc<QueueFamily> {
        &self.family
    }

    crate fn flags(&self) -> vk::QueueFlags {
        self.family.flags()
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

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            self.table.destroy_device(ptr::null());
        }
    }
}

macro_rules! check_for_features {
    ($expected:expr, $actual:expr; $($member:ident,)*) => {
        $(
            if ($expected.$member == vk::TRUE) & ($actual.$member != vk::TRUE)
            {
                Err(concat!(
                    "graphics device missing required feature:",
                    stringify!($member),
                ))?;
            }
        )*
    }
}

unsafe fn check_for_features(
    it: &vkl::InstanceTable,
    pdev: vk::PhysicalDevice,
    _desired_features: &vk::PhysicalDeviceFeatures,
    desired_descriptor_indexing_features:
        &vk::PhysicalDeviceDescriptorIndexingFeaturesEXT,
) -> Result<(), AnyError> {
    let mut descriptor_indexing_features =
        vk::PhysicalDeviceDescriptorIndexingFeaturesEXT::default();
    let mut features = vk::PhysicalDeviceFeatures2 {
        p_next: &mut descriptor_indexing_features as *mut _ as _,
        ..Default::default()
    };
    it.get_physical_device_features_2(pdev, &mut features);

    // TODO: Add boolean methods to Vk*Features in vulkan bindings
    check_for_features!(
        desired_descriptor_indexing_features, descriptor_indexing_features;
        shader_sampled_image_array_non_uniform_indexing,
        descriptor_binding_sampled_image_update_after_bind,
        descriptor_binding_update_unused_while_pending,
        descriptor_binding_partially_bound,
        runtime_descriptor_array,
    );

    Ok(())
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
            vk::EXT_DESCRIPTOR_INDEXING_EXTENSION_NAME,
        ];

        let features = Default::default();
        let descriptor_indexing_features =
            vk::PhysicalDeviceDescriptorIndexingFeaturesEXT {
                shader_sampled_image_array_non_uniform_indexing: vk::TRUE,
                descriptor_binding_sampled_image_update_after_bind: vk::TRUE,
                descriptor_binding_update_unused_while_pending: vk::TRUE,
                descriptor_binding_partially_bound: vk::TRUE,
                runtime_descriptor_array: vk::TRUE,
                ..Default::default()
            };
        check_for_features
            (it, pdev, &features, &descriptor_indexing_features)?;

        let queue_infos = [vk::DeviceQueueCreateInfo {
            queue_family_index: 0,
            queue_count: 1,
            p_queue_priorities: &1f32,
            ..Default::default()
        }];

        let create_info = vk::DeviceCreateInfo {
            p_next: &descriptor_indexing_features as *const _ as _,
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
        let mut mem_props: Box<vk::PhysicalDeviceMemoryProperties> =
            Default::default();
        it.get_physical_device_memory_properties(pdev, &mut *mem_props);

        let device = Arc::new(Device {
            instance,
            app_info,
            pdev,
            props,
            mem_props,
            table,
        });

        let queues = device.get_queues();

        Ok((device, queues))
    }

    unsafe fn get_queues(self: &Arc<Self>) -> Vec<Vec<Arc<Queue>>> {
        let props = self.instance.get_queue_family_properties(self.pdev);

        let family = Arc::new(QueueFamily {
            index: 0,
            properties: props[0],
        });

        let mut inner = vk::null();
        self.table.get_device_queue(0, 0, &mut inner);

        let queue = Arc::new(Queue {
            device: Arc::clone(self),
            inner,
            family,
            mutex: Mutex::new(()),
        });

        vec![vec![queue]]
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

    crate unsafe fn create_swapchain(self: &Arc<Self>, surface: &Arc<Surface>) ->
        Result<Arc<Swapchain>, AnyError>
    {
        Ok(Arc::new(Swapchain::new(Arc::clone(surface), Arc::clone(self))?))
    }
}
