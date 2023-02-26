use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use derivative::Derivative;
use log::{debug, trace};

use crate::*;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Surface {
    // TODO: Technically requires ownership/Arc of window. Maybe make
    // a generic type?
    #[derivative(Debug = "ignore")]
    pub(crate) instance: Arc<Instance>,
    pub(crate) inner: vk::SurfaceKHR,
}

#[derive(Debug)]
pub struct Swapchain {
    pub(crate) surface: Arc<Surface>,
    pub(crate) device: Arc<Device>,
    pub(crate) inner: vk::SwapchainKHR,
    pub(crate) extent: Extent2D,
    pub(crate) images: Vec<vk::Image>,
    views: Vec<Arc<SwapchainView>>,
    token: Token,
    name: Option<String>,
}

/// Specialized image view for the swapchain.
#[allow(dead_code)]
#[derive(Debug)]
pub struct SwapchainView {
    token: Token,
    device: Arc<Device>,
    extent: Extent2D,
    index: u32,
    inner: vk::ImageView,
}

/// Token that can be used to invalidate swapchain images.
#[derive(Clone, Debug)]
struct Token {
    inner: Arc<AtomicBool>,
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            self.instance
                .table
                .destroy_surface_khr(self.inner, ptr::null());
        }
    }
}

impl Surface {
    #[inline]
    pub unsafe fn new(instance: Arc<Instance>, window: &impl Window) -> DeviceResult<Self> {
        let inner = window.create_surface(&instance)?;
        Ok(Surface { instance, inner })
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe {
            self.destroy();
        }
    }
}

impl Swapchain {
    pub unsafe fn new(surface: Arc<Surface>, device: Arc<Device>) -> DeviceResult<Self> {
        let mut result = Swapchain {
            surface,
            device,
            inner: vk::null(),
            extent: Default::default(),
            images: Vec::new(),
            views: Vec::new(),
            token: Default::default(),
            name: None,
        };
        result.recreate()?;

        Ok(result)
    }

    #[inline]
    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    #[inline]
    pub fn inner(&self) -> vk::SwapchainKHR {
        self.inner
    }

    #[inline]
    pub fn surface(&self) -> &Arc<Surface> {
        &self.surface
    }

    #[inline]
    pub fn extent(&self) -> Extent2D {
        self.extent
    }

    #[inline]
    pub fn rect(&self) -> vk::Rect2D {
        vk::Rect2D::new(vk::Offset2D::new(0, 0), self.extent.into())
    }

    #[inline]
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
        self.device
            .table
            .destroy_swapchain_khr(self.inner, ptr::null());
    }

    pub unsafe fn recreate(&mut self) -> DeviceResult<()> {
        let dt = &*self.device.table;
        let it = &*self.device.instance.table;
        let pdev = self.device.pdev;

        let mut caps: vk::SurfaceCapabilitiesKHR = Default::default();
        it.get_physical_device_surface_capabilities_khr(pdev, self.surface.inner, &mut caps)
            .check()?;

        let max_image_count = if caps.max_image_count == 0 {
            u32::max_value()
        } else {
            caps.max_image_count
        };
        let min_image_count = std::cmp::min(caps.min_image_count + 1, max_image_count);

        // TODO: Is this a compatibility concern?
        let format = vk::Format::B8G8R8A8_SRGB;
        let color_space = vk::ColorSpaceKHR::SRGB_NONLINEAR_KHR;
        let formats = vk::enumerate2!(
            it,
            get_physical_device_surface_formats_khr,
            pdev,
            self.surface.inner,
        )?;
        if !formats
            .iter()
            .any(|fmt| fmt.format == format && fmt.color_space == color_space)
        {
            Err(err_msg!("surface format not supported"))?;
        }

        // FIXME: On Wayland, the surface extent is defined by the
        // application, so caller needs to pass us the window extent.
        // (Should be obtained from winit or whatever lib is used.)
        assert_ne!(caps.current_extent, (0xffff_ffff, 0xffff_ffff).into());

        // This can happen when a window is minimized, so don't try to
        // create a swapchain for a minimized window.
        assert_ne!(caps.current_extent, (0, 0).into());

        self.extent = caps.current_extent.into();

        let composite_alpha = vk::CompositeAlphaFlagsKHR::OPAQUE_BIT_KHR;
        if !caps.supported_composite_alpha.intersects(composite_alpha) {
            Err(err_msg!("swapchain composite alpha mode not available"))?;
        }

        let image_usage =
            vk::ImageUsageFlags::COLOR_ATTACHMENT_BIT | vk::ImageUsageFlags::TRANSFER_DST_BIT;
        if !caps.supported_usage_flags.contains(image_usage) {
            Err(err_msg!("swapchain image usage not supported"))?;
        }

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
        dt.create_swapchain_khr(&create_info, ptr::null(), &mut new)
            .check()?;

        self.destroy();
        self.inner = new;
        self.images = vk::enumerate2!(dt, get_swapchain_images_khr, self.inner)?;
        self.create_views();

        Ok(())
    }

    #[inline]
    fn create_views(&mut self) {
        self.views = (0..self.images.len())
            .map(|index| Arc::new(SwapchainView::new(&self, index as _)))
            .collect();
    }

    #[inline]
    pub fn views(&self) -> &'_ [Arc<SwapchainView>] {
        &self.views
    }

    #[inline]
    pub fn format(&self) -> Format {
        Format::BGRA8_SRGB
    }

    /// Note: A suboptimal swapchain will just return an error with no
    /// swapchain index.
    #[inline]
    pub fn acquire_next_image(&mut self, sem: &mut BinarySemaphore) -> Result<u32, vk::Result> {
        self.acquire_next_image_with_timeout(sem, u64::max_value())
    }

    pub fn acquire_next_image_with_timeout(
        &mut self,
        sem: &mut BinarySemaphore,
        timeout: u64,
    ) -> Result<u32, vk::Result> {
        trace!(
            "Swapchain::acquire_next_image_with_timeout(timeout: {})",
            timeout
        );
        let dt = &*self.device.table;
        let mut idx = 0;
        unsafe {
            dt.acquire_next_image_khr(self.inner, timeout, sem.raw(), vk::null(), &mut idx)
                .check()?;
        };
        debug!("acquired swapchain image {}", idx);
        Ok(idx)
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        let name: String = name.into();
        self.name = Some(name.clone());
        unsafe {
            self.device().set_name(self.inner(), name);
        }
    }
}

impl Named for Swapchain {
    fn name(&self) -> Option<&str> {
        Some(self.name.as_ref()?)
    }
}

impl Drop for SwapchainView {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            dt.destroy_image_view(self.inner, ptr::null());
        }
    }
}

impl SwapchainView {
    fn new(swapchain: &Swapchain, index: u32) -> Self {
        let dt = &*swapchain.device.table;
        let create_info = vk::ImageViewCreateInfo {
            image: swapchain.images[index as usize],
            view_type: vk::ImageViewType::_2D,
            format: swapchain.format().into(),
            components: Default::default(),
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR_BIT,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            },
            ..Default::default()
        };
        let mut view = vk::null();
        unsafe {
            dt.create_image_view(&create_info, ptr::null(), &mut view)
                .check()
                .unwrap();
        }

        SwapchainView {
            token: swapchain.token.clone(),
            device: Arc::clone(&swapchain.device),
            extent: swapchain.extent,
            index,
            inner: view,
        }
    }

    #[inline]
    pub fn inner(&self) -> vk::ImageView {
        self.inner
    }

    #[inline]
    pub fn extent(&self) -> Extent2D {
        self.extent
    }

    #[inline]
    pub fn format(&self) -> Format {
        Format::BGRA8_SRGB
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.token.is_valid()
    }
}

impl Token {
    #[allow(dead_code)]
    fn invalidate(&mut self) {
        self.inner.store(false, Ordering::Relaxed);
        *self = Default::default();
    }

    fn is_valid(&self) -> bool {
        self.inner.load(Ordering::Relaxed)
    }
}

impl Default for Token {
    fn default() -> Self {
        Self {
            inner: Arc::new(AtomicBool::new(true)),
        }
    }
}

pub unsafe fn device_for_surface(surface: &Surface) -> DeviceResult<vk::PhysicalDevice> {
    let instance = &*surface.instance;
    let surface = surface.inner;

    let pdevices = instance.get_physical_devices();
    for pd in pdevices.into_iter() {
        let qf = 0u32;
        let props = instance.get_queue_family_properties(pd)[qf as usize];
        let required_bits = vk::QueueFlags::GRAPHICS_BIT
            | vk::QueueFlags::COMPUTE_BIT
            | vk::QueueFlags::TRANSFER_BIT;
        if !props.queue_flags.contains(required_bits) {
            continue;
        }

        let mut surface_supp = 0;
        instance
            .table
            .get_physical_device_surface_support_khr(pd, qf, surface, &mut surface_supp)
            .check()?;
        if surface_supp != vk::TRUE {
            continue;
        }

        return Ok(pd);
    }

    Err(err_msg!("no presentable graphics device"))
}

/// Helper function which creates a logical device capable of rendering
/// to a window.
pub fn init_device_and_swapchain(
    app_info: AppInfo,
    window: &impl Window,
) -> DeviceResult<(Swapchain, Vec<Vec<Arc<Queue>>>)> {
    unsafe {
        let entrypoint = crate::loader::load_vulkan().map_err(|_| "Failed to load libvulkan")?;
        let instance = Arc::new(Instance::new(
            entrypoint,
            app_info,
            window.required_extensions(),
        )?);
        let surface = Arc::new(Surface::new(Arc::clone(&instance), window)?);
        let pdev = device_for_surface(&surface).unwrap();
        let (device, queues) = Device::new(instance, pdev)?;
        Ok((device.create_swapchain(surface)?, queues))
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::*;

    #[test]
    fn view_test() {
        let _ = TestVars::new();
    }
}
