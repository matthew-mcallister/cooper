//! This module implements a single-threaded wrapper around GLFW.

#![feature(arbitrary_self_types)]
#![feature(non_exhaustive)]
#![feature(optin_builtin_traits)]

#[cfg(test)]
macro_rules! test_type {
    () => { unit::PlainTest }
}

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::rc::Rc;

use derive_more::*;
use prelude::*;
use unit::*;

#[inline(always)]
fn bool2int(b: bool) -> c_int {
    if b { glfw::TRUE } else { glfw::FALSE }
}

#[inline(always)]
fn int2bool(i: c_int) -> bool {
    i == glfw::TRUE
}

/// An error caused by the window/input system.
#[derive(Clone, Debug, Display)]
#[display(fmt = "{}", desc)]
pub struct Error {
    code: c_int,
    desc: Box<String>,
}

impl std::error::Error for Error {}

#[derive(Clone, Constructor, Copy, Debug, From, Into)]
pub struct Dimensions {
    pub width: c_int,
    pub height: c_int,
}

impl From<Dimensions> for vk::Extent2D {
    fn from(dims: Dimensions) -> Self {
        vk::Extent2D {
            width: dims.width as u32,
            height: dims.height as u32,
        }
    }
}

/// Initializes and terminates GLFW.
#[derive(Debug)]
#[non_exhaustive]
pub struct System;

impl !Sync for System {}
impl !Send for System {}

impl Drop for System {
    fn drop(&mut self) {
        unsafe { glfw::terminate(); }
    }
}

static mut LAST_ERROR: Option<Error> = None;

unsafe extern "C" fn error_cb(code: c_int, desc: *const c_char) {
    assert!(!desc.is_null());
    let desc = Box::new(CStr::from_ptr(desc).to_str().unwrap().to_owned());
    LAST_ERROR = Some(Error { code, desc });
}

unsafe fn last_error() -> Option<Error> {
    LAST_ERROR.take()
}

impl System {
    /// This function should (allegedly) only be called from the main
    /// thread.
    pub unsafe fn init() -> Result<Rc<Self>, Error> {
        glfw::set_error_callback(Some(error_cb as _));
        if !int2bool(glfw::init()) {
            Err(last_error().unwrap())
        } else {
            Ok(Rc::new(System))
        }
    }

    pub fn vulkan_supported(&self) -> bool {
        unsafe { int2bool(glfw::vulkan_supported()) }
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
            int2bool(glfw::get_physical_device_presentation_support(
                instance.0 as _,
                physical_device.0 as _,
                queue_family,
            ))
        }
    }

    pub fn pfn_get_instance_proc_addr(&self) -> vk::pfn::GetInstanceProcAddr {
        unsafe {
            std::mem::transmute(glfw::get_instance_proc_address(
                0 as _,
                c_str!("vkGetInstanceProcAddr"),
            ))
        }
    }

    pub fn create_window(self: &Rc<Self>, info: CreateInfo) ->
        Result<Window, Error>
    {
        Window::new(Rc::clone(self), info)
    }
}

#[derive(Clone, Debug)]
pub struct CreateInfo {
    pub title: String,
    pub dims: Dimensions,
    pub hints: Hints,
}

#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub struct Hints {
    pub resizable: bool,
    pub hidden: bool,
}

/// Wrapper around the GLFW window type.
#[derive(Debug)]
pub struct Window {
    inner: ptr::NonNull<glfw::Window>,
    sys: Rc<System>,
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe { glfw::destroy_window(self.inner.as_ptr()); }
    }
}

impl Window {
    pub fn new(sys: Rc<System>, info: CreateInfo) -> Result<Self, Error> {
        unsafe {
            let title = CString::new(info.title).unwrap();
            glfw::window_hint(glfw::CLIENT_API, glfw::NO_API);
            glfw::window_hint(glfw::RESIZABLE, bool2int(info.hints.resizable));
            glfw::window_hint(glfw::VISIBLE, bool2int(!info.hints.hidden));
            let inner = glfw::create_window(
                info.dims.width,
                info.dims.height,
                title.as_ptr(),
                0 as _,
                0 as _,
            );
            let inner = ptr::NonNull::new(inner)
                .ok_or_else(|| last_error().unwrap())?;
            Ok(Window { inner, sys })
        }
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
        unsafe { int2bool(glfw::window_should_close(self.inner.as_ptr())) }
    }

    pub fn set_title(&self, title: impl Into<String>) {
        let title = CString::new(title.into()).unwrap();
        unsafe { glfw::set_window_title(self.inner.as_ptr(), title.as_ptr()); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn smoke_test() {
        let system = unsafe { System::init().unwrap() };

        let config = CreateInfo {
            title: "smoke test".to_owned(),
            dims: (320, 200).into(),
            hints: Default::default(),
        };
        let window = Window::new(system, config).unwrap();

        assert!(!window.should_close());

        window.set_title("tset ekoms");
    }

    fn error_test() {
        let system = unsafe { System::init().unwrap() };

        let config = CreateInfo {
            title: "error test".to_owned(),
            dims: (-1, -1).into(),
            hints: Default::default(),
        };
        Window::new(system, config).unwrap();
    }

    declare_tests![
        smoke_test,
        (#[should_err] error_test),
    ];
}

collect_tests![tests];

#[cfg(test)]
pub fn main() {
    let mut builder = TestDriverBuilder::new();
    crate::__collect_tests(&mut builder);
    let mut driver = builder.build(Box::new(PlainTestContext::new()));
    driver.run();
}
