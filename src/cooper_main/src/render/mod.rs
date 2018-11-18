use std::error::Error;
use std::os::raw::c_char;
use std::ptr;
use std::sync::Arc;

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
crate struct VulkanSys {
    crate config: VulkanConfig,
    crate _ws: crate::window::System,
    crate entry: vkl::Entry,
    crate inst: vkl::InstanceTable,
    crate pdev: vk::PhysicalDevice,
    crate dev: vkl::DeviceTable,
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

fn get_required_device_extensions() -> &'static [*const c_char] {
    &[vk::KHR_SWAPCHAIN_EXTENSION_NAME as *const _ as _]
}

impl VulkanSys {
    crate unsafe fn new(config: VulkanConfig) -> Result<Self, Box<dyn Error>> {
        let _ws = crate::window::System::new()?;

        if glfw::vulkan_supported() != glfw::TRUE {
            Err("Vulkan not supported")?;
        }

        let get_instance_proc_addr = std::mem::transmute({
            glfw::get_instance_proc_address
                (0 as _, c_str!("vkGetInstanceProcAddr"))
        });
        let entry = vkl::Entry::load(get_instance_proc_addr);

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
        let inst = vkl::InstanceTable::load(inst, get_instance_proc_addr);

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
        let exts = get_required_device_extensions();
        let create_info = vk::DeviceCreateInfo {
            s_type: vk::StructureType::DEVICE_CREATE_INFO,
            p_next: ptr::null(),
            flags: Default::default(),
            queue_create_info_count: 1,
            p_queue_create_infos: &queue_create_info as _,
            enabled_layer_count: 0,
            pp_enabled_layer_names: ptr::null(),
            enabled_extension_count: exts.len() as _,
            pp_enabled_extension_names: exts.as_ptr(),
            p_enabled_features: &features as _,
        };
        let mut dev = vk::null();
        inst.create_device(pdev, &create_info as _, ptr::null(), &mut dev as _)
            .check()?;

        let get_device_proc_addr = std::mem::transmute(get_instance_proc_addr
            (inst.instance, c_str!("vkGetDeviceProcAddr")).unwrap());
        let dev = vkl::DeviceTable::load(dev, get_device_proc_addr);

        let mut queue = vk::null();
        dev.get_device_queue(0, 0, &mut queue as _);

        Ok(VulkanSys {
            config, _ws, entry, inst, pdev, dev, queue,
        })
    }
}

crate struct VulkanSwapchain {
    crate sys: Arc<VulkanSys>,
    crate surface: vk::SurfaceKhr,
    crate swapchain: vk::SwapchainKhr,
}

impl Drop for VulkanSwapchain {
    fn drop(&mut self) {
        unsafe {
            self.sys.dev.destroy_swapchain_khr(self.swapchain, ptr::null());
            self.sys.inst.destroy_surface_khr(self.surface, ptr::null());
        }
    }
}

impl VulkanSwapchain {
    crate unsafe fn new(sys: Arc<VulkanSys>, window: &crate::window::Window) ->
        Result<Self, Box<dyn Error>>
    {
        let mut surface: vk::SurfaceKhr = vk::null();
        let res = glfw::create_window_surface(
            sys.inst.instance.0 as _,
            window.inner.as_ptr(),
            0 as *const _,
            &mut surface.0 as _,
        );
        vk::Result(res).check()?;

        let mut result = VulkanSwapchain {
            sys, surface, swapchain: vk::null(),
        };
        result.recreate_swapchain()?;

        Ok(result)
    }

    crate unsafe fn recreate_swapchain(&mut self) -> Result<(), Box<dyn Error>>
    {
        let mut caps: vk::SurfaceCapabilitiesKhr = Default::default();
        self.sys.inst.get_physical_device_surface_capabilities_khr
            (self.sys.pdev, self.surface, &mut caps as _).check()?;

        let min_image_count = if caps.max_image_count > 0 {
            u32::min(caps.min_image_count + 1, caps.max_image_count)
        } else { caps.min_image_count + 1 };

        // The spec says that, on Wayland (and probably other platforms,
        // maybe embedded), the surface extent may be determined by the
        // swapchain extent rather than the other way around.
        if (0xffffffff, 0xffffffff) == caps.current_extent.into()
            { Err("surface extent undefined")?; }

        // TODO: The spec says that you are unable to create a swapchain
        // when this happens. Which platforms do this?
        if (0, 0) == caps.current_extent.into()
            { Err("surface has zero extent")?; }

        let formats = vk_enumerate2!(
            self.sys.inst,
            get_physical_device_surface_formats_khr,
            self.sys.pdev,
            self.surface,
        )?;
        // The first option seems to be best for most common drivers
        let vk::SurfaceFormatKhr { format, color_space } = formats[0];

        let composite_alpha = vk::CompositeAlphaFlagsKhr::OPAQUE_BIT_KHR;
        if !caps.supported_composite_alpha.intersects(composite_alpha)
            { Err("opaque composite alpha mode unavailable")?; }

        let image_usage
            = vk::ImageUsageFlags::COLOR_ATTACHMENT_BIT
            | vk::ImageUsageFlags::TRANSFER_DST_BIT;
        if !caps.supported_usage_flags.contains(image_usage)
            { Err("surface image usage requirements unmet")?; }

        let create_info = vk::SwapchainCreateInfoKhr {
            s_type: vk::StructureType::SWAPCHAIN_CREATE_INFO_KHR,
            p_next: ptr::null(),
            flags: Default::default(),
            surface: self.surface,
            min_image_count,
            image_format: format,
            image_color_space: color_space,
            image_extent: caps.current_extent,
            image_array_layers: 1,
            image_usage,
            image_sharing_mode: vk::SharingMode::EXCLUSIVE,
            queue_family_index_count: 0,
            p_queue_family_indices: ptr::null(),
            pre_transform: caps.current_transform,
            composite_alpha,
            present_mode: vk::PresentModeKhr::FIFO_KHR,
            clipped: vk::TRUE,
            old_swapchain: self.swapchain,
        };
        self.sys.dev.create_swapchain_khr
            (&create_info as _, ptr::null(), &mut self.swapchain as _)
            .check()?;

        Ok(())
    }
}
