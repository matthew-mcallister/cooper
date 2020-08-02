use std::ffi::CString;
use std::ptr;
use std::sync::Arc;

use prelude::*;

use crate::*;

// TODO: Give a debug name to this
pub struct Device {
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

// The hash and eq impls operate solely on the VkDevice handle, which
// should be a globally unique pointer.
impl std::hash::Hash for Device {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.table.device.hash(state);
    }
}

impl PartialEq for Device {
    fn eq(&self, other: &Self) -> bool {
        self.table.device == other.table.device
    }
}

impl Eq for Device {}

impl std::fmt::Debug for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Device")
            .field("pdev", &self.pdev)
            .finish()
    }
}

impl Device {
    pub unsafe fn new(instance: Arc<Instance>, pdev: vk::PhysicalDevice) ->
        Result<(Arc<Self>, Vec<Vec<Arc<Queue>>>), AnyError>
    {
        let it = &instance.table;
        let app_info = Arc::clone(&instance.app_info);

        let mut p_next = ptr::null_mut();

        // TODO: check that extensions are actually supported
        let exts = [
            vk::KHR_SWAPCHAIN_EXTENSION_NAME,
        ];

        let features = vk::PhysicalDeviceFeatures {
            image_cube_array: vk::TRUE, // Currently only used in tests
            sampler_anisotropy: vk::TRUE,
            ..Default::default()
        };
        let mut features12 = vk::PhysicalDeviceVulkan12Features {
            timeline_semaphore: vk::TRUE,
            ..Default::default()
        };
        add_to_pnext!(p_next, features12);

        let queue_infos = [vk::DeviceQueueCreateInfo {
            queue_family_index: 0,
            queue_count: 1,
            p_queue_priorities: &1f32,
            ..Default::default()
        }];

        let create_info = vk::DeviceCreateInfo {
            p_next,
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

        let queues = Queue::get_device_queues(&device);

        Ok((device, queues))
    }

    #[inline]
    pub fn table(&self) -> &vkl::DeviceTable {
        &self.table
    }

    #[inline]
    pub fn instance(&self) -> &Arc<Instance> {
        &self.instance
    }

    #[inline]
    pub fn properties(&self) -> &vk::PhysicalDeviceProperties {
        &self.props
    }

    #[inline]
    pub fn queue_family<'dev>(
        self: &'dev Arc<Self>,
        index: u32,
    ) -> QueueFamily<'dev> {
        QueueFamily::new(self, index)
    }

    #[inline]
    pub fn limits(&self) -> &vk::PhysicalDeviceLimits {
        &self.properties().limits
    }

    #[inline]
    pub fn features(&self) -> &vk::PhysicalDeviceFeatures {
        &self.features
    }

    pub unsafe fn set_name(
        &self,
        handle: impl DebugHandle,
        name: impl Into<String>,
    ) {
        if self.app_info.debug {
            let name = CString::new(name.into()).unwrap();
            set_name(&self.table, handle, &name);
        }
    }

    pub unsafe fn create_swapchain(self: Arc<Self>, surface: Arc<Surface>)
        -> Result<Swapchain, AnyError>
    {
        let mut swapchain = Swapchain::new(surface, self)?;
        set_name!(swapchain);
        Ok(swapchain)
    }

    pub fn wait_idle(&self) {
        unsafe { self.table.device_wait_idle(); }
    }
}
