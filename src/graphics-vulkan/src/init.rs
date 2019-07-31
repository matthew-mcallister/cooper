// TODO: User-friendly errors
use std::ffi::CString;
use std::os::raw::c_char;
use std::ptr;
use std::sync::Arc;

use common::*;

use crate::*;

#[derive(Debug, Default)]
pub struct GraphicsConfig {
    pub app_name: String,
    pub app_version: [u32; 3],
    pub enable_debug_names: bool,
}

#[derive(Debug)]
pub struct Instance {
    pub wsys: window::System,
    pub entry: Arc<vkl::Entry>,
    pub table: Arc<vkl::InstanceTable>,
    pub config: Arc<GraphicsConfig>,
}

impl Drop for Instance {
    fn drop(&mut self) {
        unsafe { self.table.destroy_instance(ptr::null()); }
    }
}

impl Instance {
    pub unsafe fn new(config: Arc<GraphicsConfig>) -> Result<Self, AnyError> {
        let wsys = window::System::new()?;
        if !wsys.vulkan_supported() { Err("vulkan not available")?; }

        let get_instance_proc_addr = wsys.pfn_get_instance_proc_addr();
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

        // TODO: Detect if required extensions are unavailable
        let mut extensions = Vec::new();
        extensions.extend(wsys.required_instance_extensions());
        if config.enable_debug_names {
            extensions.push(vk::EXT_DEBUG_UTILS_EXTENSION_NAME);
        }

        let create_info = vk::InstanceCreateInfo {
            p_application_info: &app_info as _,
            enabled_extension_count: extensions.len() as _,
            pp_enabled_extension_names: extensions.as_ptr(),
            ..Default::default()
        };

        let mut inst = vk::null();
        entry.create_instance(&create_info as _, ptr::null(), &mut inst as _)
            .check()?;
        let table =
            Arc::new(vkl::InstanceTable::load(inst, get_instance_proc_addr));

        Ok(Instance { wsys, entry, table, config })
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
        self.table.get_physical_device_properties(pdev, &mut *res as _);
        res
    }
}

#[derive(Debug)]
pub struct Surface {
    pub instance: Arc<Instance>,
    pub window: Arc<window::Window>,
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
            instance,
            window,
            inner,
        })
    }
}

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
            (pd, qf, surface, &mut surface_supp as _)
            .check()?;
        if surface_supp != vk::TRUE { continue; }

        return Ok(pd);
    }

    Err("no presentable graphics device".into())
}

#[derive(Debug)]
pub struct Device {
    pub instance: Arc<Instance>,
    pub config: Arc<GraphicsConfig>,
    pub pdev: vk::PhysicalDevice,
    pub props: Box<vk::PhysicalDeviceProperties>,
    pub mem_props: Box<vk::PhysicalDeviceMemoryProperties>,
    pub table: Arc<vkl::DeviceTable>,
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { self.table.destroy_device(ptr::null()); }
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
    it.get_physical_device_features_2(pdev, &mut features as _);

    // TODO: Add logic methods to Vk*Features in vulkan bindings
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
        Result<Self, AnyError>
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
            p_queue_priorities: &1f32 as _,
            ..Default::default()
        }];

        let create_info = vk::DeviceCreateInfo {
            p_next: &descriptor_indexing_features as *const _ as _,
            queue_create_info_count: queue_infos.len() as _,
            p_queue_create_infos: queue_infos.as_ptr(),
            enabled_extension_count: exts.len() as _,
            pp_enabled_extension_names: exts.as_ptr(),
            p_enabled_features: &features as _,
            ..Default::default()
        };
        let mut dev = vk::null();
        it.create_device(pdev, &create_info as _, ptr::null(), &mut dev as _)
            .check()?;

        let get_device_proc_addr = std::mem::transmute({
            it.get_instance_proc_addr(c_str!("vkGetDeviceProcAddr"))
        });
        let table =
            Arc::new(vkl::DeviceTable::load(dev, get_device_proc_addr));

        let props = instance.get_properties(pdev);
        let mut mem_props: Box<vk::PhysicalDeviceMemoryProperties> =
            Default::default();
        it.get_physical_device_memory_properties(pdev, &mut *mem_props as _);

        Ok(Device {
            instance,
            config,
            pdev,
            props,
            mem_props,
            table,
        })
    }

    pub unsafe fn get_queue(&self, qf: u32, idx: u32) -> vk::Queue {
        let mut queue = vk::null();
        self.table.get_device_queue(qf, idx, &mut queue as _);
        queue
    }

    pub unsafe fn set_debug_name<T: CanDebug>(
        &self,
        obj: T,
        name: *const c_char,
    ) {
        if self.config.enable_debug_names {
            set_debug_name(&self.table, obj, name);
        }
    }
}

#[derive(Debug)]
pub struct Swapchain {
    pub surface: Arc<Surface>,
    pub device: Arc<Device>,
    pub inner: vk::SwapchainKHR,
    pub format: vk::Format,
    pub extent: vk::Extent2D,
    pub images: Vec<vk::Image>,
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe { self.destroy(); }
    }
}

impl Swapchain {
    pub unsafe fn new(surface: Arc<Surface>, device: Arc<Device>) ->
        Result<Self, AnyError>
    {
        let mut result = Swapchain {
            surface,
            device,
            inner: vk::null(),
            format: Default::default(),
            extent: Default::default(),
            images: Vec::new(),
        };
        result.recreate()?;

        Ok(result)
    }

    pub fn rectangle(&self) -> vk::Rect2D {
        vk::Rect2D::new(vk::Offset2D::new(0, 0), self.extent)
    }

    unsafe fn destroy(&self) {
        self.device.table.destroy_swapchain_khr(self.inner, ptr::null());
    }

    pub unsafe fn recreate(&mut self) -> Result<(), AnyError> {
        let dt = &self.device.table;
        let it: &vkl::InstanceTable = &self.device.instance.table;
        let pdev = self.device.pdev;

        let mut caps: vk::SurfaceCapabilitiesKHR = Default::default();
        it.get_physical_device_surface_capabilities_khr
            (pdev, self.surface.inner, &mut caps as _)
            .check()?;

        let max_image_count =
            if caps.max_image_count == 0 { u32::max_value() }
            else { caps.max_image_count };
        let min_image_count =
            std::cmp::min(caps.min_image_count + 1, max_image_count);

        // TODO: Is this a compatibility concern?
        let format = vk::Format::B8G8R8A8_SRGB;
        let color_space = vk::ColorSpaceKHR::SRGB_NONLINEAR_KHR;
        let formats = vk::enumerate2!(
            it,
            get_physical_device_surface_formats_khr,
            pdev,
            self.surface.inner,
        )?;
        if !formats.iter().any(|fmt| fmt.format == format &&
            fmt.color_space == color_space)
        {
            Err("surface format not supported")?;
        }

        self.format = format;

        // The spec says that, on Wayland (and probably other platforms,
        // maybe embedded), the surface extent may be determined by the
        // swapchain extent rather than the other way around.
        if (0xffff_ffff, 0xffff_ffff) == caps.current_extent.into()
            { Err("surface extent undefined")?; }

        // TODO: The spec says that you are unable to create a swapchain
        // when this happens. Which platforms do this?
        if (0, 0) == caps.current_extent.into()
            { Err("surface has zero extent")?; }

        self.extent = caps.current_extent;

        let composite_alpha = vk::CompositeAlphaFlagsKHR::OPAQUE_BIT_KHR;
        if !caps.supported_composite_alpha.intersects(composite_alpha)
            { Err("swapchain composite alpha mode not available")?; }

        let image_usage
            = vk::ImageUsageFlags::COLOR_ATTACHMENT_BIT
            | vk::ImageUsageFlags::TRANSFER_DST_BIT;
        if !caps.supported_usage_flags.contains(image_usage)
            { Err("swapchain image usage not supported")?; }

        let create_info = vk::SwapchainCreateInfoKHR {
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
        };
        let mut new = vk::null();
        dt.create_swapchain_khr
            (&create_info as _, ptr::null(), &mut new as _).check()?;

        self.destroy();
        self.inner = new;
        self.images = vk::enumerate2!
            (dt, get_swapchain_images_khr, self.inner)?;

        Ok(())
    }
}
