// TODO: User-friendly errors
use std::ffi::CString;
use std::ptr;
use std::sync::Arc;

use derivative::Derivative;
use log::{debug, info, trace};
use more_asserts::assert_ge;
use prelude::*;

use crate::*;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Instance {
    crate vk: window::VulkanPlatform,
    #[derivative(Debug = "ignore")]
    crate entry: Arc<vkl::Entry>,
    #[derivative(Debug = "ignore")]
    crate table: Arc<vkl::InstanceTable>,
    crate app_info: Arc<AppInfo>,
    debug_messengers: Vec<DebugMessenger>,
    debug_handler: Arc<DefaultDebugMessageHandler>,
}

#[derive(Clone, Debug, Default)]
pub struct AppInfo {
    pub name: String,
    pub version: [u32; 3],
    pub debug: bool,
    pub test: bool,
}

impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            for messenger in self.debug_messengers.iter_mut() {
                messenger.destroy(&self.table);
            }
            trace!("Instance::drop()");
            self.table.destroy_instance(ptr::null());
        }

        // XXX: Don't bother if already panicking
        if self.app_info.test {
            let msg_count = self.debug_message_count();
            assert!(msg_count == 0, "caught {} validation errors", msg_count);
        }
    }
}

impl Instance {
    pub unsafe fn new(vk: window::VulkanPlatform, app_info: AppInfo) ->
        Result<Self, AnyError>
    {
        if !vk.supported() { Err("vulkan not available")?; }

        let get_instance_proc_addr = vk.pfn_get_instance_proc_addr();
        let entry = Arc::new(vkl::Entry::load(get_instance_proc_addr));

        let mut version = 0;
        entry.enumerate_instance_version(&mut version).check().unwrap();
        let version = unpack_version(version);
        assert_ge!(version, [1, 2, 0]);
        debug!("Vulkan version: {:?}", version);

        let name = CString::new(app_info.name.clone()).unwrap();
        let [major, minor, patch] = app_info.version;
        let vk_app_info = vk::ApplicationInfo {
            p_application_name: name.as_ptr(),
            application_version: vk::make_version!(major, minor, patch),
            api_version: vk::API_VERSION_1_2,
            p_engine_name: c_str!("cooper"),
            engine_version: vk::make_version!(0, 1, 0),
            ..Default::default()
        };

        // TODO: Detect if required layers/extensions are unavailable
        let mut layers = Vec::new();
        let mut extensions = Vec::new();
        extensions.extend(vk.required_instance_extensions());

        if app_info.debug {
            layers.push(c_str!("VK_LAYER_KHRONOS_validation"));
            extensions.push(vk::EXT_DEBUG_UTILS_EXTENSION_NAME);
        }

        info!("enabled layers: {:?}", debug_cstrs(&layers));
        info!("enabled instance extensions: {:?}",
            debug_cstrs(&extensions));

        let create_info = vk::InstanceCreateInfo {
            p_application_info: &vk_app_info,
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

        let app_info = Arc::new(app_info);
        let mut instance = Instance {
            vk,
            entry,
            table,
            app_info,
            debug_messengers: Vec::new(),
            debug_handler: Default::default(),
        };

        if instance.app_info.test {
            let severity
                = vk::DebugUtilsMessageSeverityFlagsEXT::WARNING_BIT_EXT
                | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR_BIT_EXT;
            let ty
                = vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION_BIT_EXT
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE_BIT_EXT;
            let handler = Arc::clone(&instance.debug_handler);
            instance.register_debug_messenger(severity, ty, handler);
        }

        Ok(instance)
    }

    #[inline]
    pub fn app_info(&self) -> &AppInfo {
        &self.app_info
    }

    pub unsafe fn get_physical_devices(&self) -> Vec<vk::PhysicalDevice> {
        vk::enumerate2!(self.table, enumerate_physical_devices).unwrap()
    }

    pub unsafe fn get_queue_family_properties(
        &self,
        pdev: vk::PhysicalDevice,
    ) -> Vec<vk::QueueFamilyProperties> {
        vk::enumerate2!(
            @void self.table,
            get_physical_device_queue_family_properties,
            pdev,
        )
    }

    pub unsafe fn get_properties(&self, pdev: vk::PhysicalDevice) ->
        vk::PhysicalDeviceProperties
    {
        let mut res = Default::default();
        self.table.get_physical_device_properties(pdev, &mut res);
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

    crate unsafe fn register_debug_messenger(
        &mut self,
        severity: vk::DebugUtilsMessageSeverityFlagsEXT,
        types: vk::DebugUtilsMessageTypeFlagsEXT,
        handler: Arc<dyn DebugMessageHandler>,
    ) {
        let messenger = DebugMessenger::new(self, severity, types, handler);
        self.debug_messengers.push(messenger);
    }

    fn debug_message_count(&self) -> u32 {
        self.debug_handler.message_count()
    }
}

fn unpack_version(version: u32) -> [u32; 3] {
    [
        vk::version_major!(version),
        vk::version_minor!(version),
        vk::version_patch!(version),
    ]
}
