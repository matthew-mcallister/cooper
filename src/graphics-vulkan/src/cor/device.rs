use std::ffi::CStr;
use std::ptr;
use std::sync::{Arc, Mutex};

use prelude::*;

use crate::*;

pub unsafe fn device_for_surface(surface: &Surface) ->
    Result<vk::PhysicalDevice, AnyError>
{
    let instance = &*surface.instance;
    let surface = surface.inner;

    let pdevices = instance.get_physical_devices();
    for pd in pdevices.into_iter() {
        let qf = 0u32;
        let props = instance.get_queue_family_properties(pd)[qf as usize];
        let required_bits = vk::QueueFlags::GRAPHICS_BIT
            | vk::QueueFlags::COMPUTE_BIT
            | vk::QueueFlags::TRANSFER_BIT;
        if !props.queue_flags.contains(required_bits) { continue; }

        let mut surface_supp = 0;
        instance.table.get_physical_device_surface_support_khr
            (pd, qf, surface, &mut surface_supp).check()?;
        if surface_supp != vk::TRUE { continue; }

        return Ok(pd);
    }

    Err("no presentable graphics device".into())
}

#[derive(Debug)]
pub struct Device {
    pub instance: Arc<Instance>,
    pub config: Arc<InitConfig>,
    pub pdev: vk::PhysicalDevice,
    pub props: Box<vk::PhysicalDeviceProperties>,
    pub mem_props: Box<vk::PhysicalDeviceMemoryProperties>,
    pub table: Arc<vkl::DeviceTable>,
}

#[derive(Debug)]
pub struct QueueFamily {
    pub index: u32,
    pub properties: vk::QueueFamilyProperties,
}

#[derive(Debug)]
pub struct Queue {
    pub device: Arc<Device>,
    pub inner: vk::Queue,
    pub family: Arc<QueueFamily>,
    mutex: Mutex<()>,
}

impl Queue {
    pub unsafe fn submit(
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

    pub unsafe fn present(&self, present_info: &vk::PresentInfoKHR) ->
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
    pub unsafe fn new(instance: Arc<Instance>, pdev: vk::PhysicalDevice) ->
        Result<(Arc<Self>, Vec<Vec<Arc<Queue>>>), AnyError>
    {
        let it = &instance.table;
        let config = Arc::clone(&instance.config);

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
            config,
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

    pub unsafe fn set_debug_name<T, A>(&self, obj: T, name: A)
    where
        T: DebugUtils,
        A: AsRef<CStr>,
    {
        if self.config.debug {
            set_debug_name(&self.table, obj, name.as_ref().as_ptr());
        }
    }

    pub unsafe fn create_fence(&self, signaled: bool) -> vk::Fence {
        let mut create_info = vk::FenceCreateInfo::default();
        if signaled {
            create_info.flags |= vk::FenceCreateFlags::SIGNALED_BIT;
        }
        let mut obj = vk::null();
        self.table.create_fence(&create_info, ptr::null(), &mut obj)
            .check().unwrap();
        obj
    }

    pub unsafe fn create_semaphore(&self) -> vk::Semaphore {
        let create_info = Default::default();
        let mut obj = vk::null();
        self.table.create_semaphore(&create_info, ptr::null(), &mut obj)
            .check().unwrap();
        obj
    }

    pub unsafe fn create_swapchain(self: &Arc<Self>, surface: &Arc<Surface>) ->
        Result<Arc<Swapchain>, AnyError>
    {
        Ok(Arc::new(Swapchain::new(Arc::clone(surface), Arc::clone(self))?))
    }
}
