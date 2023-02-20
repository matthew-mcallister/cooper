use crate::Instance;

/// Trait to create a surface for a particular window system.
pub trait Window {
    /// Returns a list of extension names required to create the
    /// surface.
    fn required_extensions(&self) -> &'static [&'static str];

    /// Creates a surface attached to this window.
    fn create_surface(&self, instance: &Instance) -> Result<vk::SurfaceKHR, vk::Result>;
}

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
))]
impl Window for winit::window::Window {
    fn required_extensions(&self) -> &'static [&'static str] {
        return &[
            "VK_KHR_surface",
            "VK_KHR_wayland_surface",
            "VK_KHR_xlib_surface",
        ];
    }

    fn create_surface(&self, instance: &Instance) -> Result<vk::SurfaceKHR, vk::Result> {
        use winit::platform::x11::WindowExtX11;
        use winit::platform::wayland::WindowExtWayland;
        let mut handle: vk::SurfaceKHR = vk::null();
        if let Some(surface) = self.wayland_surface() {
            let info = vk::WaylandSurfaceCreateInfoKHR {
                display: self.wayland_display().unwrap() as _,
                surface: surface as _,
                ..Default::default()
            };
            unsafe {
                instance
                    .table
                    .create_wayland_surface_khr(&info, std::ptr::null(), &mut handle)
                    .check()?;
            }
        } else if let Some(window) = self.xlib_window() {
            let info = vk::XlibSurfaceCreateInfoKHR {
                dpy: self.xlib_display().unwrap() as _,
                window: window as _,
                ..Default::default()
            };
            unsafe {
                instance
                    .table
                    .create_xlib_surface_khr(&info, std::ptr::null(), &mut handle)
                    .check()?;
            }
        }
        Ok(handle)
    }
}
