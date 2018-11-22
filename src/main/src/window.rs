use std::os::raw::c_char;
use std::ptr;

use crate::glfw;
use crate::vk;

// An error caused by the windowing system. GLFW reports errors through
// callbacks, so the provided message will come from the application and
// not GLFW itself. The message is instead meant to help locate where an
// error occurred in the application.
#[derive(Clone, Constructor, Copy, Debug)]
crate struct Error {
    msg: &'static str,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Constructor, Copy, Debug)]
crate struct Dimensions {
    crate width: i32,
    crate height: i32,
}

impl From<Dimensions> for vk::Extent2D {
    fn from(dims: Dimensions) -> Self {
        vk::Extent2D {
            width: dims.width as u32,
            height: dims.height as u32,
        }
    }
}

static mut GLFW_USE_COUNT: u32 = 0;

// Reference-counting wrapper to make sure GLFW is automatically
// initialized and terminated.
#[non_exhaustive]
#[derive(Debug)]
crate struct System {}

impl System {
    crate unsafe fn new() -> Result<Self, Error> {
        if glfw::init() != glfw::TRUE {
            Err(Error::new("failed to initialize GLFW"))
        } else {
            GLFW_USE_COUNT += 1;
            Ok(System {})
        }
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
            System {}
        }
    }
}

// Wrapper around the GLFW window type.
#[derive(Debug)]
crate struct Window {
    crate inner: ptr::NonNull<glfw::Window>,
    sys: System,
}

impl Window {
    crate unsafe fn new(
        dims: Dimensions,
        title: *const c_char,
        // TODO: select monitor/fullscreen
    ) -> Result<Self, Error> {
        let sys = System::new()?;
        glfw::window_hint(glfw::CLIENT_API, glfw::NO_API);
        let inner = glfw::create_window
            (dims.width, dims.height, title, 0 as _, 0 as _);
        let inner = ptr::NonNull::new(inner)
            .ok_or(Error::new("failed to create window"))?;
        Ok(Window { inner, sys })
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe { glfw::destroy_window(self.inner.as_ptr()); }
    }
}
