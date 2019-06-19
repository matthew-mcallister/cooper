//! This module implements a fairly thin wrapper around GLFW.
#![feature(optin_builtin_traits)]
use std::os::raw::c_char;
use std::ptr;

use derive_more::*;

macro_rules! c_str {
    ($str:expr) => {
        concat!($str, "\0") as *const str as *const std::os::raw::c_char
    }
}

/// An error caused by the windowing system. GLFW reports errors through
/// callbacks, so the provided message will indicate where the error
/// occurred in application source rather than in GLFW.
#[derive(Clone, Constructor, Copy, Debug)]
pub struct Error {
    msg: &'static str,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Constructor, Copy, Debug, From, Into)]
pub struct Dimensions {
    pub width: i32,
    pub height: i32,
}

impl From<Dimensions> for vk::Extent2D {
    fn from(dims: Dimensions) -> Self {
        vk::Extent2D {
            width: dims.width as u32,
            height: dims.height as u32,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub title: *const c_char,
    pub dims: Dimensions,
}

static mut GLFW_USE_COUNT: u32 = 0;

/// Reference-counting wrapper to make sure GLFW is automatically
/// initialized and terminated. Not thread safe.
#[derive(Debug)]
pub struct System { _priv: () }

impl System {
    pub unsafe fn new() -> Result<Self, Error> {
        if glfw::init() != glfw::TRUE {
            Err(Error::new("failed to initialize GLFW"))
        } else {
            GLFW_USE_COUNT += 1;
            Ok(System { _priv: () })
        }
    }

    pub fn vulkan_supported(&self) -> bool {
        unsafe { glfw::vulkan_supported() == glfw::TRUE }
    }

    pub fn required_instance_extensions(&self) -> &[*const c_char] {
        let mut count: u32 = 0;
        unsafe {
            let ptr = glfw::get_required_instance_extensions(&mut count as _);
            std::slice::from_raw_parts(ptr, count as _)
        }
    }

    pub fn queue_family_supports_present(
        &self,
        instance: vk::Instance,
        physical_device: vk::PhysicalDevice,
        queue_family: u32,
    ) -> bool {
        unsafe {
            glfw::get_physical_device_presentation_support(
                instance.0 as _,
                physical_device.0 as _,
                queue_family,
            ) == glfw::TRUE
        }
    }

    pub fn pfn_get_instance_proc_addr(&self) ->
        vk::pfn::GetInstanceProcAddr
    {
        unsafe {
            std::mem::transmute(glfw::get_instance_proc_address(
                0 as _,
                c_str!("vkGetInstanceProcAddr"),
            ))
        }
    }

    pub fn poll_events(&self) {
        unsafe { glfw::poll_events(); }
    }
}

impl !Send for System {}
impl !Sync for System {}

impl Drop for System {
    fn drop(&mut self) {
        unsafe {
            GLFW_USE_COUNT -= 1;
            if GLFW_USE_COUNT == 0 { glfw::terminate(); }
        }
    }
}

impl Clone for System {
    fn clone(&self) -> Self {
        unsafe {
            GLFW_USE_COUNT += 1;
            System { _priv: () }
        }
    }
}

/// Wrapper around the GLFW window type.
#[derive(Debug)]
pub struct Window {
    inner: ptr::NonNull<glfw::Window>,
    sys: System,
}

impl Window {
    pub fn sys(&self) -> &System { &self.sys }

    pub unsafe fn new(sys: System, config: Config) -> Result<Self, Error> {
        // TODO: select monitor/fullscreen
        glfw::window_hint(glfw::CLIENT_API, glfw::NO_API);
        let inner = glfw::create_window(
            config.dims.width,
            config.dims.height,
            config.title,
            0 as _,
            0 as _,
        );
        let inner = ptr::NonNull::new(inner)
            .ok_or(Error::new("failed to create window"))?;
        Ok(Window { inner, sys })
    }

    pub unsafe fn create_surface(&self, instance: vk::Instance) ->
        Result<vk::SurfaceKHR, vk::Result>
    {
        let mut surface = vk::null();
        vk::Result(glfw::create_window_surface(
            instance.0 as _,
            self.inner.as_ptr(),
            0 as _,
            &mut surface as *mut _ as _,
        )).check()?;
        Ok(surface)
    }

    pub fn should_close(&self) -> bool {
        unsafe {
            glfw::window_should_close(self.inner.as_ptr()) == glfw::TRUE
        }
    }

    pub fn set_title(&self, title: *const c_char) {
        unsafe {
            glfw::set_window_title(self.inner.as_ptr(), title);
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe { glfw::destroy_window(self.inner.as_ptr()); }
    }
}
