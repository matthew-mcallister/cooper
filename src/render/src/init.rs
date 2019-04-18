//! This module includes the renderer initialization routine and the
//! interaction with the window system.
use std::error::Error;
use std::os::raw::c_char;
use std::ptr;
use std::sync::Arc;

use derivative::Derivative;

const VALIDATION_LAYER: *const c_char =
    c_str!("VK_LAYER_LUNARG_standard_validation");

#[derive(Clone, Debug)]
pub struct Config {
    pub enable_validation: bool,
}

#[derive(Debug)]
pub struct Instance {
    crate _config: Config,
    crate table: Arc<vkl::InstanceTable>,
}

impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            self.table.destroy_instance(ptr::null());
        }
    }
}

fn get_required_device_extensions() -> &'static [*const c_char] {
    &[vk::KHR_SWAPCHAIN_EXTENSION_NAME as *const _ as _]
}

impl Instance {
    pub unsafe fn new(config: Config) -> Result<Self, Box<dyn Error>> {
        let wsys = window::System::new()?;

        if !wsys.vulkan_supported() {
            Err("Vulkan not supported")?;
        }

        let get_instance_proc_addr = wsys.pfn_get_instance_proc_addr();
        let entry = vkl::Entry::load(get_instance_proc_addr);

        let layers =
            if config.enable_validation { &[VALIDATION_LAYER][..] }
            else { &[][..] };
        let exts = wsys.required_extensions();

        let app_info = vk::ApplicationInfo {
            s_type: vk::StructureType::APPLICATION_INFO,
            p_next: ptr::null(),
            p_application_name: c_str!("cooper"),
            application_version: vk::make_version!(0, 1, 0),
            p_engine_name: c_str!("cooper"),
            engine_version: vk::make_version!(0, 1, 0),
            api_version: vk::API_VERSION_1_0,
        };
        let create_info = vk::InstanceCreateInfo {
            s_type: vk::StructureType::INSTANCE_CREATE_INFO,
            p_next: ptr::null(),
            flags: Default::default(),
            p_application_info: &app_info as _,
            enabled_layer_count: layers.len() as _,
            pp_enabled_layer_names: layers.as_ptr(),
            enabled_extension_count: exts.len() as _,
            pp_enabled_extension_names: exts.as_ptr(),
        };
        let mut inst = vk::null();
        entry.create_instance(&create_info as _, ptr::null(), &mut inst as _)
            .check()?;
        let table = vkl::InstanceTable::load(inst, get_instance_proc_addr);
        let table = Arc::new(table);

        Ok(Instance { _config: config, table })
    }
}

#[derive(Debug)]
pub struct Surface {
    crate instance: Arc<Instance>,
    crate win: Arc<window::Window>,
    crate inner: vk::SurfaceKHR,
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            self.instance.table.destroy_surface_khr(self.inner, ptr::null());
        }
    }
}

impl Surface {
    pub fn new(instance: Arc<Instance>, win: Arc<window::Window>) ->
        Result<Self, vk::Result>
    {
        let inner: vk::SurfaceKHR =
            unsafe { win.create_surface(instance.table.instance)? };
        Ok(Surface { instance, win, inner })
    }
}

#[derive(Debug)]
pub struct Device {
    crate instance: Arc<Instance>,
    crate pdev: vk::PhysicalDevice,
    crate table: Arc<vkl::DeviceTable>,
    crate queue_family: u32,
    crate queue: vk::Queue,
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { self.table.destroy_device(ptr::null()); }
    }
}

impl Device {
    pub unsafe fn new(surface: &Surface) -> Result<Self, Box<dyn Error>> {
        let it: &vkl::InstanceTable = &surface.instance.table;
        let instance = it.instance;

        let pdevices = vk::enumerate2!(it, enumerate_physical_devices)?;
        let (pdev, qf, _) = pdevices.into_iter()
            .flat_map(|pd| {
                // Iterate over all (pdevice, queue_family) pairs
                let qf_props = vk::enumerate2!(
                    @void it,
                    get_physical_device_queue_family_properties,
                    pd,
                );
                qf_props.into_iter()
                    .enumerate()
                    .map(move |(idx, props)| (pd, idx as u32, props))
            })
            .find(|&(pd, idx, props)| {
                let required_bits = vk::QueueFlags::GRAPHICS_BIT
                    | vk::QueueFlags::COMPUTE_BIT
                    | vk::QueueFlags::TRANSFER_BIT;
                if !props.queue_flags.contains(required_bits) { return false; }

                if !surface.win.sys()
                    .queue_family_supports_present(instance, pd, idx)
                {
                    return false;
                }

                let mut surface_supp = 0;
                it.get_physical_device_surface_support_khr
                    (pd, idx, surface.inner, &mut surface_supp as _)
                    .check().unwrap();
                surface_supp == vk::TRUE
            }).ok_or("no presentable graphics device")?;

        let queue_create_info = vk::DeviceQueueCreateInfo {
            s_type: vk::StructureType::DEVICE_QUEUE_CREATE_INFO,
            p_next: ptr::null(),
            flags: Default::default(),
            queue_family_index: qf,
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
        it.create_device(pdev, &create_info as _, ptr::null(), &mut dev as _)
            .check()?;

        let get_device_proc_addr = std::mem::transmute({
            it.get_instance_proc_addr(c_str!("vkGetDeviceProcAddr")).unwrap()
        });
        let table =
            Arc::new(vkl::DeviceTable::load(dev, get_device_proc_addr));

        let mut queue = vk::null();
        table.get_device_queue(qf, 0, &mut queue as _);

        Ok(Device {
            instance: Arc::clone(&surface.instance),
            pdev,
            table,
            queue_family: qf,
            queue,
        })
    }

    #[allow(dead_code)]
    pub unsafe fn create_shader_module(&self, src: &[u8]) ->
        Result<vk::ShaderModule, vk::Result>
    {
        assert_eq!(src.len() % 4, 0);
        let create_info = vk::ShaderModuleCreateInfo {
            s_type: vk::StructureType::SHADER_MODULE_CREATE_INFO,
            p_next: ptr::null(),
            flags: Default::default(),
            code_size: src.len() as _,
            p_code: src.as_ptr() as _,
        };
        let mut sm = vk::null();
        self.table.create_shader_module
            (&create_info, ptr::null(), &mut sm as _).check()?;
        Ok(sm)
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Swapchain {
    crate surface: Arc<Surface>,
    crate dev: Arc<Device>,
    crate dt: Arc<vkl::DeviceTable>,
    crate inner: vk::SwapchainKHR,
    #[derivative(Debug = "ignore")]
    crate create_info: Box<vk::SwapchainCreateInfoKHR>,
    crate images: Vec<vk::Image>,
    crate image_views: Vec<vk::ImageView>,
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe { self.destroy(); }
    }
}

impl Swapchain {
    pub unsafe fn new(surface: Arc<Surface>, dev: Arc<Device>) ->
        Result<Self, Box<dyn Error>>
    {
        let mut result = Swapchain {
            surface,
            dt: Arc::clone(&dev.table),
            dev,
            inner: vk::null(),
            create_info: Default::default(),
            images: Vec::new(),
            image_views: Vec::new(),
        };
        result.recreate()?;

        Ok(result)
    }

    unsafe fn destroy(&self) {
        for &view in self.image_views.iter()
            { self.dt.destroy_image_view(view, ptr::null()); }
        self.dt.destroy_swapchain_khr(self.inner, ptr::null());
    }

    pub unsafe fn recreate(&mut self) -> Result<(), Box<dyn Error>> {
        self.destroy();

        let it: &vkl::InstanceTable = &self.dev.instance.table;
        let pdev = self.dev.pdev;

        let mut caps: vk::SurfaceCapabilitiesKHR = Default::default();
        it.get_physical_device_surface_capabilities_khr
            (pdev, self.surface.inner, &mut caps as _)
            .check()?;

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

        let composite_alpha = vk::CompositeAlphaFlagsKHR::OPAQUE_BIT_KHR;
        if !caps.supported_composite_alpha.intersects(composite_alpha)
            { Err("swapchain composite alpha mode not available")?; }

        let image_usage
            = vk::ImageUsageFlags::COLOR_ATTACHMENT_BIT
            | vk::ImageUsageFlags::TRANSFER_DST_BIT;
        if !caps.supported_usage_flags.contains(image_usage)
            { Err("swapchain image usage not supported")?; }

        let formats = vk::enumerate2!(
            self.dev.instance.table,
            get_physical_device_surface_formats_khr,
            pdev,
            self.surface.inner,
        )?;
        // The first option seems to be best for most common drivers
        let vk::SurfaceFormatKHR { format, color_space } = formats[0];

        self.create_info = Box::new(vk::SwapchainCreateInfoKHR {
            s_type: vk::StructureType::SWAPCHAIN_CREATE_INFO_KHR,
            p_next: ptr::null(),
            flags: Default::default(),
            surface: self.surface.inner,
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
            present_mode: vk::PresentModeKHR::FIFO_KHR,
            clipped: vk::TRUE,
            old_swapchain: self.inner,
        });
        self.dt.create_swapchain_khr
            (&*self.create_info as _, ptr::null(), &mut self.inner as _)
            .check()?;

        self.images = vk::enumerate2!(
            self.dt,
            get_swapchain_images_khr,
            self.inner,
        )?;

        for &image in self.images.iter() {
            let create_info = vk::ImageViewCreateInfo {
                s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
                p_next: ptr::null(),
                flags: Default::default(),
                image,
                view_type: vk::ImageViewType::_2D,
                format,
                components: Default::default(),
                subresource_range: vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR_BIT,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                },
            };
            let mut image_view = vk::null();
            self.dt.create_image_view
                (&create_info as _, ptr::null(), &mut image_view as _)
                .check()?;
            self.image_views.push(image_view);
        }

        Ok(())
    }
}
