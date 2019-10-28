use std::ptr;
use std::sync::Arc;

use prelude::*;

use crate::*;

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

    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    pub fn inner(&self) -> vk::SwapchainKHR {
        self.inner
    }

    pub fn rect(&self) -> vk::Rect2D {
        vk::Rect2D::new(vk::Offset2D::new(0, 0), self.extent)
    }

    pub fn viewport(&self) -> vk::Viewport {
        vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: self.extent.width as _,
            height: self.extent.height as _,
            min_depth: 1.0,
            max_depth: 0.0,
        }
    }

    unsafe fn destroy(&self) {
        self.device.table.destroy_swapchain_khr(self.inner, ptr::null());
    }

    pub unsafe fn recreate(&mut self) -> Result<(), AnyError> {
        let dt = &*self.device.table;
        let it: &vkl::InstanceTable = &self.device.instance.table;
        let pdev = self.device.pdev;

        let mut caps: vk::SurfaceCapabilitiesKHR = Default::default();
        it.get_physical_device_surface_capabilities_khr
            (pdev, self.surface.inner, &mut caps)
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

        // FIXME: On Wayland, the surface extent is defined by the
        // application, so we need to pull window dimensions from config
        // rather than the surface object.
        assert_ne!(caps.current_extent, (0xffff_ffff, 0xffff_ffff).into());

        // This can happen when a window is minimized, so don't try to
        // create a swapchain for a minimized window.
        assert_ne!(caps.current_extent, (0, 0).into());

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
            clipped: vk::FALSE,
            old_swapchain: self.inner,
        };
        let mut new = vk::null();
        dt.create_swapchain_khr(&create_info, ptr::null(), &mut new).check()?;

        self.destroy();
        self.inner = new;
        self.images = vk::enumerate2!
            (dt, get_swapchain_images_khr, self.inner)?;

        Ok(())
    }
}

crate unsafe fn init_swapchain(
    app_info: AppInfo,
    window: Arc<window::Window>
) -> Result<(Swapchain, Vec<Vec<Arc<Queue>>>), AnyError> {
    let vk_platform = window.vk_platform().clone();
    let instance = Arc::new(Instance::new(vk_platform, app_info)?);
    let surface = Arc::new(Surface::new(Arc::clone(&instance), window)?);
    let pdev = device_for_surface(&surface).unwrap();
    let (device, queues) = Device::new(instance, pdev)?;
    Ok((Swapchain::new(surface, device)?, queues))
}
