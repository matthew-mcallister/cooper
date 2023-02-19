use crate::Device;

/// Trait to create a surface for a particular window system.
pub trait Window {
    /// Returns a list of extension names required to create the
    /// surface.
    fn required_extensions() -> &'static [&'static str];

    /// Creates a surface attached to this window.
    fn create_surface(&self, device: &Device) -> vk::Surface;
}

impl Window for winit::window::Window {
    #[cfg(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    )]
    unsafe fn required_extensions() -> &'static [&'static str] {
        // TODO: Shouldn't require both wayland and xlib?
        return &[
            "VK_KHR_surface",
            "VK_KHR_wayland_surface",
            "VK_KHR_xlib_surface",
        ];
    }

    fn create_surface(&self, device: &Device) -> vk::Surface {
        use winit::platform::unix::WindowExtUnix;
        if let Some(handle) = self.wayland_surface() {
            let info = vk::WaylandSurfaceCreateInfo {};
        } else if let Some(handle) = self.xlib_window() {
            let info = vk::XlibSurfaceCreateInfoKHR {
                dpy: self.xlib_display().unwrap(),
                window: handle,
                ..Default::default()
            };
        }
    }
}
