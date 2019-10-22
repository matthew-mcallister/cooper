// TODO: User-friendly errors
use std::ffi::CString;
use std::ptr;
use std::sync::Arc;

use prelude::*;

use crate::*;

#[derive(Debug, Default)]
pub struct InitConfig {
    pub app_name: String,
    pub app_version: [u32; 3],
    pub debug: bool,
}

#[derive(Debug)]
pub struct Instance {
    pub vk: window::VulkanPlatform,
    pub entry: Arc<vkl::Entry>,
    pub table: Arc<vkl::InstanceTable>,
    pub config: Arc<InitConfig>,
}

impl Drop for Instance {
    fn drop(&mut self) {
        unsafe { self.table.destroy_instance(ptr::null()); }
    }
}

impl Instance {
    pub unsafe fn new(
        vk: window::VulkanPlatform,
        config: InitConfig,
    ) -> Result<Self, AnyError> {
        if !vk.supported() { Err("vulkan not available")?; }

        let get_instance_proc_addr = vk.pfn_get_instance_proc_addr();
        let entry = Arc::new(vkl::Entry::load(get_instance_proc_addr));

        let app_name = CString::new(config.app_name.clone()).unwrap();
        let [major, minor, patch] = config.app_version;
        let app_info = vk::ApplicationInfo {
            p_application_name: app_name.as_ptr(),
            application_version: vk::make_version!(major, minor, patch),
            api_version: vk::API_VERSION_1_1,
            p_engine_name: c_str!("cooper"),
            engine_version: vk::make_version!(0, 1, 0),
            ..Default::default()
        };

        // TODO: Detect if required layers/extensions are unavailable
        let mut layers = Vec::new();
        let mut extensions = Vec::new();
        extensions.extend(vk.required_instance_extensions());

        if config.debug {
            layers.push(c_str!("VK_LAYER_KHRONOS_validation"));
            extensions.push(vk::EXT_DEBUG_UTILS_EXTENSION_NAME);
        }

        let create_info = vk::InstanceCreateInfo {
            p_application_info: &app_info,
            enabled_layer_count: layers.len() as _,
            pp_enabled_layer_names: layers.as_ptr(),
            enabled_extension_count: extensions.len() as _,
            pp_enabled_extension_names: extensions.as_ptr(),
            ..Default::default()
        };

        let mut inst = vk::null();
        entry.create_instance(&create_info, ptr::null(), &mut inst).check()?;
        let table =
            Arc::new(vkl::InstanceTable::load(inst, get_instance_proc_addr));

        let config = Arc::new(config);
        Ok(Instance { vk, entry, table, config })
    }

    pub unsafe fn get_physical_devices(&self) -> Vec<vk::PhysicalDevice> {
        vk::enumerate2!(self.table, enumerate_physical_devices).unwrap()
    }

    pub unsafe fn get_queue_family_properties(&self, pdev: vk::PhysicalDevice)
        -> Vec<vk::QueueFamilyProperties>
    {
        vk::enumerate2!(
            @void self.table,
            get_physical_device_queue_family_properties,
            pdev,
        )
    }

    pub unsafe fn get_properties(&self, pdev: vk::PhysicalDevice) ->
        Box<vk::PhysicalDeviceProperties>
    {
        let mut res = Box::new(Default::default());
        self.table.get_physical_device_properties(pdev, &mut *res);
        res
    }

    pub unsafe fn create_device(self: &Arc<Self>, pdev: vk::PhysicalDevice) ->
        Result<(Arc<Device>, Vec<Vec<Arc<Queue>>>), AnyError>
    {
        Ok(Device::new(Arc::clone(self), pdev)?)
    }

    pub unsafe fn create_surface(
        self: &Arc<Self>,
        window: &Arc<window::Window>,
    ) -> Result<Arc<Surface>, AnyError> {
        Ok(Arc::new(Surface::new(Arc::clone(self), Arc::clone(window))?))
    }
}

#[derive(Debug)]
pub struct Surface {
    pub window: Arc<window::Window>,
    pub instance: Arc<Instance>,
    pub inner: vk::SurfaceKHR,
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            self.instance.table.destroy_surface_khr(self.inner, ptr::null());
        }
    }
}

impl Surface {
    pub unsafe fn new(instance: Arc<Instance>, window: Arc<window::Window>) ->
        Result<Self, AnyError>
    {
        let inner = window.create_surface(instance.table.instance)?;
        Ok(Surface {
            window,
            instance,
            inner,
        })
    }
}

#[cfg(test)]
mod tests {
    fn smoke_test(_vars: crate::testing::TestVars) {
        // Do nothing
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
