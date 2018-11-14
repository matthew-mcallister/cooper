use std::error::Error;
use std::os::raw::c_char;
use std::ptr;

use crate::glfw;
use crate::vk;
use crate::vkl;

const VALIDATION_LAYER: *const c_char =
    c_str!("VK_LAYER_LUNARG_standard_validation");

#[derive(Clone, Debug)]
crate struct VulkanConfig {
    crate enable_validation: bool,
}

// Stores the products of initializing Vulkan
// NB: The pointer tables are relatively large.
crate struct VulkanSys {
    crate config: VulkanConfig,
    crate ws: crate::window::System,
    crate entry: vkl::Entry,
    crate inst: vkl::CoreInstance,
    crate pdev: vk::PhysicalDevice,
    crate dev: vkl::CoreDevice,
    crate queue: vk::Queue,
}

impl Drop for VulkanSys {
    fn drop(&mut self) {
        unsafe {
            self.dev.destroy_device(ptr::null());
            self.inst.destroy_instance(ptr::null());
        }
    }
}

#[derive(Clone)]
crate struct VulkanWindowConfig {
    crate extent: vk::Extent2D,
}

crate struct VulkanWindowState {
    crate config: VulkanWindowConfig,
    crate surface_api: vkl::KhrSurface,
    crate surface: vk::SurfaceKhr,
    crate swapchain: vk::SwapchainKhr,
}

impl VulkanSys {
    crate unsafe fn new(config: VulkanConfig) -> Result<Self, Box<dyn Error>>
    {
        let ws = crate::window::System::new()?;

        if glfw::vulkan_supported() != glfw::TRUE {
            Err("Vulkan not supported")?;
        }

        let get_instance_proc_addr = std::mem::transmute({
            glfw::get_instance_proc_address
                (0 as _, c_str!("vkGetInstanceProcAddr"))
        });
        let entry = vkl::Entry::load(get_instance_proc_addr).unwrap();

        let layers =
            if config.enable_validation { &[VALIDATION_LAYER][..] }
            else { &[][..] };

        let mut num_exts: u32 = 0;
        let exts =
            glfw::get_required_instance_extensions(&mut num_exts as _);

        let app_info = vk::ApplicationInfo {
            s_type: vk::StructureType::APPLICATION_INFO,
            p_next: ptr::null(),
            p_application_name: c_str!("cooper"),
            application_version: vk_make_version!(0, 1, 0),
            p_engine_name: c_str!("cooper"),
            engine_version: vk_make_version!(0, 1, 0),
            api_version: vk::API_VERSION_1_0,
        };
        let create_info = vk::InstanceCreateInfo {
            s_type: vk::StructureType::INSTANCE_CREATE_INFO,
            p_next: ptr::null(),
            flags: Default::default(),
            p_application_info: &app_info as _,
            enabled_layer_count: layers.len() as _,
            pp_enabled_layer_names: layers.as_ptr(),
            enabled_extension_count: num_exts,
            pp_enabled_extension_names: exts,
        };
        let mut inst = vk::null();
        entry.create_instance(&create_info as _, ptr::null(), &mut inst as _)
            .check()?;
        let inst = vkl::CoreInstance::load(inst, get_instance_proc_addr)?;

        let pdevices = vk_enumerate2!(inst, enumerate_physical_devices)?;
        let pdev = pdevices.into_iter().find(|pd| {
            // NB: future hardware may support presentation on a queue
            // family other than the first and this will no longer work
            glfw::TRUE == glfw::get_physical_device_presentation_support
                (inst.instance.0 as _, pd.0 as _, 0)
        }).ok_or("no presentable graphics device")?;

        let queue_create_info = vk::DeviceQueueCreateInfo {
            s_type: vk::StructureType::DEVICE_QUEUE_CREATE_INFO,
            p_next: ptr::null(),
            flags: Default::default(),
            queue_family_index: 0,
            queue_count: 1,
            p_queue_priorities: &1.0f32 as _,
        };
        let features: vk::PhysicalDeviceFeatures = Default::default();
        let create_info = vk::DeviceCreateInfo {
            s_type: vk::StructureType::DEVICE_CREATE_INFO,
            p_next: ptr::null(),
            flags: Default::default(),
            queue_create_info_count: 1,
            p_queue_create_infos: &queue_create_info as _,
            enabled_layer_count: 0,
            pp_enabled_layer_names: ptr::null(),
            enabled_extension_count: 0,
            pp_enabled_extension_names: ptr::null(),
            p_enabled_features: &features as _,
        };
        let mut dev = vk::null();
        inst.create_device(pdev, &create_info as _, ptr::null(), &mut dev as _)
            .check()?;

        let get_device_proc_addr = std::mem::transmute(get_instance_proc_addr
            (inst.instance, c_str!("vkGetDeviceProcAddr")).unwrap());
        let dev = vkl::CoreDevice::load(dev, get_device_proc_addr)?;

        let mut queue = vk::null();
        dev.get_device_queue(0, 0, &mut queue as _);

        Ok(VulkanSys {
            config, ws, entry, inst, pdev, dev, queue,
        })
    }
}
