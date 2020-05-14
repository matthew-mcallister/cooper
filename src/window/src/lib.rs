//! This crate implements a multithreaded layer over GLFW to provide a
//! cross-platform interface to the host's windowing and input systems.
//! It is based on an "event loop" model where one thread acts as an
//! event handler and worker while others interact with it through proxy
//! objects.

#![feature(arbitrary_self_types)]
#![feature(negative_impls)]
#![feature(optin_builtin_traits)]

#[cfg(test)]
macro_rules! test_type {
    () => { unit::PlainTest }
}

macro_rules! get_var {
    ($exp:expr, $var:path) => {
        match $exp {
            $var(x) => Some(x),
            _ => None,
        }
    }
}

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::time::{Duration, Instant};

use base::request as rq;
use crossbeam_channel as cc;
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

static mut LAST_ERROR: Option<Error> = None;

unsafe extern "C" fn error_cb(code: c_int, desc: *const c_char) {
    assert!(!desc.is_null());
    let desc = Box::new(CStr::from_ptr(desc).to_str().unwrap().to_owned());
    LAST_ERROR = Some(Error { code, desc });
}

unsafe fn last_error() -> Option<Error> {
    LAST_ERROR.take()
}

type WindowPtr = ptr::NonNull<glfw::Window>;

/// A thin wrapper around a GLFWwindow pointer. Only the event loop
/// should access the inner pointer.
#[derive(Clone, Copy, Debug, From, Into)]
struct WindowHandle {
    inner: WindowPtr,
}

unsafe impl Send for WindowHandle {}
unsafe impl Sync for WindowHandle {}

impl WindowHandle {
    fn as_ptr(self) -> *mut glfw::Window {
        self.inner.as_ptr()
    }
}

#[derive(Debug)]
enum Request {
    Poke,
    CreateWindow {
        info: CreateInfo,
    },
    DestroyWindow {
        window: WindowHandle,
    },
    CreateSurface {
        window: WindowHandle,
        instance: vk::Instance,
    },
    SetTitle {
        window: WindowHandle,
        title: String,
    },
    WindowShouldClose {
        window: WindowHandle,
    },
}

#[derive(Debug, From)]
enum Response {
    WindowCreated(Result<WindowHandle, Error>),
    SurfaceCreated(Result<vk::SurfaceKHR, vk::Result>),
    WindowShouldClose(bool),
}

type RequestSender = rq::RequestSender<Request, Response>;

/// Creates an event loop (thus initializing GLFW) and a proxy object
/// that can safely communicate with the loop from other threads.
///
/// # Safety
///
/// For maximum platform compatibility (and maybe fewer bugs), call this
/// function from the "main" thread. It is unsafe to call twice.
pub unsafe fn init() -> Result<(EventLoop, EventLoopProxy), Error> {
    EventLoop::new()
}

/// An asynchronous worker that runs on the main thread and performs
/// API calls on behalf of other threads. Due to platform-specific
/// windowing limitations, this object must be created and remain on the
/// main thread. All actual platform API calls are made through this
/// object, never through the thread-safe proxy objects.
/// Other threads may send the main thread work requests over a shared
/// channel; the main thread can respond by "pumping" the requests in a
/// loop.
#[derive(Debug)]
#[non_exhaustive]
pub struct EventLoop {
    service: rq::Service<EventHandler>,
    ticker: cc::Receiver<Instant>,
}

impl !Sync for EventLoop {}
impl !Send for EventLoop {}

impl Drop for EventLoop {
    fn drop(&mut self) {
        // Bring down the whole program if others are still using GLFW
        // (the alternative is to leave GLFW initialized and let the
        // other threads crash after they realize the channel is closed)
        match self.service.try_pump() {
            Err(cc::RecvError) => {},
            _ => {
                const EXIT_CODE: i32 = 0xD0A;
                std::process::exit(EXIT_CODE);
            },
        }

        // Clean up
        unsafe { glfw::terminate(); }
    }
}

impl EventLoop {
    unsafe fn new() -> Result<(Self, EventLoopProxy), Error> {
        glfw::set_error_callback(Some(error_cb as _));
        if !int2bool(glfw::init()) {
            return Err(last_error().unwrap());
        }

        let (service, sender) = rq::Service::unbounded(EventHandler);

        let ticker = cc::tick(Duration::from_millis(4));
        let evt = EventLoop { service, ticker };

        let proxy = EventLoopProxy { sender };

        Ok((evt, proxy))
    }

    pub fn set_poll_interval(&mut self, interval: Duration) {
        self.ticker = cc::tick(interval);
    }

    /// Handles any pending requests and window system events.
    pub fn try_pump(&mut self) -> Result<(), cc::RecvError> {
        unsafe { glfw::poll_events(); }
        self.service.try_pump()?;
        Ok(())
    }

    /// Pumps requests and window system events according to the current
    /// polling interval. Returns the number of events handled. If the
    /// number of events is zero, it may be safe to assume the other end
    /// of the channel has stalled.
    pub fn pump_with_timeout(&mut self) -> Result<u64, cc::RecvError> {
        unsafe { glfw::poll_events(); }
        let (count, _) = self.service.pump_with_fallback(&self.ticker)?;
        Ok(count)
    }

    /// Pumps requests and window system events until the channel is
    /// disconnected.
    pub fn pump(&mut self) {
        while let Ok(_) = self.pump_with_timeout() {}
    }
}

/// Logic for responding to requests from other threads.
#[derive(Debug)]
struct EventHandler;

impl EventHandler {
    unsafe fn create_window(&self, info: CreateInfo) ->
        Result<WindowHandle, Error>
    {
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
        Ok(WindowHandle { inner })
    }

    unsafe fn create_surface(
        &self,
        window: WindowHandle,
        instance: vk::Instance,
    ) -> Result<vk::SurfaceKHR, vk::Result> {
        let mut surface = vk::null();
        vk::Result(glfw::create_window_surface(
            instance.0 as _,
            window.as_ptr(),
            0 as _,
            &mut surface as *mut _ as _,
        )).check()?;
        Ok(surface)
    }
}

impl rq::RequestHandler for EventHandler {
    type Request = Request;
    type Response = Response;

    fn handle(&mut self, req: Self::Request) -> Option<Self::Response> {
        match req {
            Request::Poke => None,
            Request::CreateWindow { info } => {
                Some(unsafe { self.create_window(info).into() })
            },
            Request::DestroyWindow { window } => {
                unsafe { glfw::destroy_window(window.as_ptr()); }
                None
            },
            Request::CreateSurface { window, instance } => {
                Some(unsafe { self.create_surface(window, instance).into() })
            },
            Request::SetTitle { window, title } => {
                unsafe {
                    let title = CString::new(title).unwrap();
                    glfw::set_window_title(window.as_ptr(), title.as_ptr());
                }
                None
            },
            Request::WindowShouldClose { window } => {
                unsafe {
                    let res = glfw::window_should_close(window.as_ptr());
                    Some(Response::WindowShouldClose(int2bool(res)))
                }
            },
        }
    }
}

/// Entrypoint to the Vulkan API and platform-independent abstraction
/// layer.
// TODO: It'd be great if this were reference-counted and not managed by
// GLFW.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct VulkanPlatform {
    sender: RequestSender,
}

impl AsRef<VulkanPlatform> for RequestSender {
    fn as_ref(&self) -> &VulkanPlatform {
        unsafe { std::mem::transmute(self) }
    }
}

impl VulkanPlatform {
    pub fn supported(&self) -> bool {
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
}

/// Thread-safe entrypoint to the window/input API that acts as proxy to
/// the underlying event loop.
///
/// This type relies on the main thread to act as a worker and respond
/// to requests for windowing operations. That is, the main thread is a
/// client of the X server (or whatever), and other threads are clients
/// of the main thread. This relationship holds for the `Window` type as
/// well. The main thread must handle these requests using the
/// `EventLoop` interface.
///
/// N.B.: In the current implementation, each time you wait for a
/// response, it blocks the current thread.
#[derive(Clone, Debug)]
pub struct EventLoopProxy {
    sender: RequestSender,
}

impl AsRef<EventLoopProxy> for RequestSender {
    fn as_ref(&self) -> &EventLoopProxy {
        unsafe { std::mem::transmute(self) }
    }
}

impl EventLoopProxy {
    pub fn vk_platform(&self) -> &VulkanPlatform {
        self.sender.as_ref()
    }

    /// Sends a message to the event loop which does nothing but
    /// increase the message counter. Doesn't wait for a response. This
    /// is useful to wake the event loop thread and let it know it isn't
    /// deadlocked.
    pub fn poke(&self) {
        self.sender.send(Request::Poke).unwrap();
    }

    pub fn create_window(&self, info: CreateInfo) -> Result<Window, Error> {
        let request = Request::CreateWindow { info };
        let response = self.sender.wait_on(request).unwrap();
        let inner = get_var!(response, Response::WindowCreated).unwrap()?;
        let sender = self.sender.clone();
        Ok(Window { inner, sender })
    }
}

#[derive(Clone, Debug)]
pub struct CreateInfo {
    pub title: String,
    pub dims: Dimensions,
    pub hints: CreationHints,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CreationHints {
    pub resizable: bool,
    pub hidden: bool,
}

/// A proxy object to a platform window created by the application
/// which allows making calls to the underlying platform API.
///
/// Note that this interface must route API calls through the main
/// thread, so methods won't succeed unless the main thread is actively
/// handling requests. See the documentation for `EventLoopProxy`.
#[derive(Debug)]
pub struct Window {
    inner: WindowHandle,
    sender: RequestSender,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl Drop for Window {
    fn drop(&mut self) {
        let request = Request::DestroyWindow { window: self.inner };
        self.sender.send(request).unwrap();
    }
}

impl Window {
    pub fn vk_platform(&self) -> &VulkanPlatform {
        self.sender.as_ref()
    }

    pub fn proxy(&self) -> &EventLoopProxy {
        self.sender.as_ref()
    }

    pub unsafe fn create_surface(&self, instance: vk::Instance) ->
        Result<vk::SurfaceKHR, vk::Result>
    {
        let request = Request::CreateSurface {
            window: self.inner,
            instance,
        };
        let response = self.sender.wait_on(request).unwrap();
        get_var!(response, Response::SurfaceCreated).unwrap()
    }

    /// Asynchronously sets the window title.
    pub fn set_title(&self, title: String) {
        let request = Request::SetTitle {
            window: self.inner,
            title,
        };
        self.sender.send(request).unwrap();
    }

    pub fn should_close(&self) -> bool {
        let request = Request::WindowShouldClose {
            window: self.inner,
        };
        let response = self.sender.wait_on(request).unwrap();
        get_var!(response, Response::WindowShouldClose).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use std::thread;
    use super::*;

    fn smoke_test() {
        let (mut evt, proxy) = unsafe { init().unwrap() };

        let thd = thread::spawn(move || {
            let config = CreateInfo {
                title: "smoke test".to_owned(),
                dims: (320, 200).into(),
                hints: CreationHints {
                    hidden: true,
                    ..Default::default()
                },
            };
            let window = proxy.create_window(config).unwrap();
            window.set_title("tset ekoms".to_owned());
        });

        evt.pump();
        thd.join().unwrap();
    }

    fn error_test() {
        let (mut evt, _) = unsafe { init().unwrap() };

        // Make sure we don't deadlock
        let thd = thread::spawn(move || panic!());

        evt.pump();
        thd.join().unwrap();
    }

    // TODO: Tests should be skipped (not fail) if GLFW is unavailable
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
    let mut driver = builder.build_basic();
    driver.run();
}
