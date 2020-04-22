use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use log::trace;
use prelude::*;

use crate::*;

#[derive(Debug)]
crate struct Surface {
    crate window: Arc<window::Window>,
    crate instance: Arc<Instance>,
    crate inner: vk::SurfaceKHR,
}

#[derive(Debug)]
crate struct Swapchain {
    crate surface: Arc<Surface>,
    crate device: Arc<Device>,
    crate inner: vk::SwapchainKHR,
    crate extent: Extent2D,
    crate images: Vec<vk::Image>,
    token: Token,
}

/// Specialized image view for the swapchain.
#[derive(Debug)]
crate struct SwapchainView {
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
            self.instance.table.destroy_surface_khr(self.inner, ptr::null());
        }
    }
}

impl Surface {
    crate unsafe fn new(instance: Arc<Instance>, window: Arc<window::Window>)
        -> Result<Self, AnyError>
    {
        let inner = window.create_surface(instance.table.instance)?;
        Ok(Surface {
            window,
            instance,
            inner,
        })
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe { self.destroy(); }
    }
}

impl Swapchain {
    crate unsafe fn new(surface: Arc<Surface>, device: Arc<Device>) ->
        Result<Self, AnyError>
    {
        let mut result = Swapchain {
            surface,
            device,
            inner: vk::null(),
            extent: Default::default(),
            images: Vec::new(),
            token: Default::default(),
        };
        result.recreate()?;

        Ok(result)
    }

    crate fn extent(&self) -> Extent2D {
        self.extent
    }

    crate fn rect(&self) -> vk::Rect2D {
        vk::Rect2D::new(vk::Offset2D::new(0, 0), self.extent.into())
    }

    crate fn viewport(&self) -> vk::Viewport {
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

    crate unsafe fn recreate(&mut self) -> Result<(), AnyError> {
        let dt = &*self.device.table;
        let it = &*self.device.instance.table;
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

        // FIXME: On Wayland, the surface extent is defined by the
        // application, so we need to pull window dimensions from config
        // rather than the surface object.
        assert_ne!(caps.current_extent, (0xffff_ffff, 0xffff_ffff).into());

        // This can happen when a window is minimized, so don't try to
        // create a swapchain for a minimized window.
        assert_ne!(caps.current_extent, (0, 0).into());

        self.extent = caps.current_extent.into();

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

    crate fn create_views(&self) -> Vec<Arc<SwapchainView>> {
        (0..self.images.len())
            .map(|index| Arc::new(SwapchainView::new(&self, index as _)))
            .collect()
    }

    crate fn format(&self) -> Format {
        Format::BGRA8_SRGB
    }

    /// Note: A suboptimal swapchain will just return an error with no
    /// swapchain index.
    // TODO: Use case for synchronizing with a fence?
    crate fn acquire_next_image(&mut self, sem: &mut Semaphore) ->
        Result<u32, vk::Result>
    {
        self.acquire_next_image_with_timeout(sem, u64::max_value())
    }

    crate fn acquire_next_image_with_timeout(
        &mut self,
        sem: &mut Semaphore,
        timeout: u64,
    ) -> Result<u32, vk::Result> {
        trace!("acquiring swapchain image");
        let dt = &*self.device.table;
        let mut idx = 0;
        unsafe {
            dt.acquire_next_image_khr(
                self.inner,
                timeout,
                sem.inner(),
                vk::null(),
                &mut idx,
            ).check()?;
        };
        trace!("acquired image {}", idx);
        Ok(idx)
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
                .check().unwrap();
        }

        SwapchainView {
            token: swapchain.token.clone(),
            device: Arc::clone(&swapchain.device),
            extent: swapchain.extent,
            index,
            inner: view,
        }
    }

    crate fn inner(&self) -> vk::ImageView {
        self.inner
    }

    crate fn extent(&self) -> Extent2D {
        self.extent
    }

    crate fn format(&self) -> Format {
        Format::BGRA8_SRGB
    }

    crate fn is_valid(&self) -> bool {
        self.token.is_valid()
    }
}

impl Token {
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
        Self { inner: Arc::new(AtomicBool::new(true)) }
    }
}

crate unsafe fn device_for_surface(surface: &Surface) ->
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

#[cfg(test)]
mod tests {
    use crate::*;

    unsafe fn view_test(vars: testing::TestVars) {
        let _attchs = vars.swapchain.create_views();
    }

    unit::declare_tests![view_test];
}

unit::collect_tests![tests];
