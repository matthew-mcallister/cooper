use std::ptr;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use cooper_graphics_vulkan::*;

#[macro_export]
macro_rules! include_shader {
    ($name:expr) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            concat!("/generated/shaders/", $name),
        ))
    }
}

pub(crate) unsafe fn init_video(ev: &window::EventLoopProxy, title: &str) ->
    (Arc<Swapchain>, Vec<Vec<Arc<Queue>>>)
{
    let create_info = window::CreateInfo {
        title: title.to_owned(),
        dims: window::Dimensions::new(1280, 720),
        hints: Default::default(),
    };
    let window = Arc::new(ev.create_window(create_info).unwrap());

    let config = GraphicsConfig {
        app_name: title.to_owned(),
        app_version: [0, 1, 0],
        debug: true,
    };
    let vk_platform = window.vk_platform().clone();
    let instance = Arc::new(Instance::new(vk_platform, config).unwrap());
    let surface = instance.create_surface(&window).unwrap();
    let pdev = device_for_surface(&surface).unwrap();
    let (device, queues) = instance.create_device(pdev).unwrap();
    let swapchain = device.create_swapchain(&surface).unwrap();

    (swapchain, queues)
}

pub(crate) unsafe fn with_event_loop<F>(f: F)
where
    F: FnOnce(window::EventLoopProxy) + Send + 'static
{
    let (mut ev_loop, ev_proxy) = window::init().unwrap();

    let thread = thread::spawn(move || f(ev_proxy));

    ev_loop.pump();
    thread.join().unwrap();
}

#[derive(Debug)]
pub(crate) struct AppResources {
    pub(crate) window: Arc<window::Window>,
    pub(crate) swapchain: Arc<Swapchain>,
    pub(crate) queues: Vec<Vec<Arc<Queue>>>,
    pub(crate) set_layouts: Arc<DescriptorSetLayoutManager>,
    pub(crate) pipe_layouts: Arc<PipelineLayoutManager>,
    pub(crate) shaders: Arc<ShaderManager>,
    pub(crate) render_passes: Arc<RenderPassManager>,
    pub(crate) framebuffers: Arc<FramebufferChain>,
}

#[derive(Debug)]
pub(crate) struct AppBase {
    pub(crate) res: Arc<AppResources>,
    pub(crate) dt: Arc<vkl::DeviceTable>,
    pub(crate) gfx_queue: Arc<Queue>,
    pub(crate) present_image: u32,
    pub(crate) acquire_sem: vk::Semaphore,
    pub(crate) render_sem: vk::Semaphore,
    pub(crate) render_fence: vk::Fence,
    pub(crate) start: Instant,
    pub(crate) frame_start: Instant,
}

impl Drop for AppBase {
    fn drop(&mut self) {
        let dt = &*self.dt;
        unsafe {
            dt.destroy_semaphore(self.acquire_sem, ptr::null());
            dt.destroy_fence(self.render_fence, ptr::null());
            dt.destroy_semaphore(self.render_sem, ptr::null());
        }
    }
}

impl AppBase {
    pub(crate) unsafe fn new(resources: Arc<AppResources>) -> Self {
        let res = resources;
        let device = &*res.swapchain.device;
        let dt = Arc::clone(&device.table);

        let gfx_queue = Arc::clone(&res.queues[0][0]);

        let acquire_sem = device.create_semaphore();
        let render_sem = device.create_semaphore();
        let render_fence = device.create_fence(true);

        let time = Instant::now();

        AppBase {
            res,
            dt,
            gfx_queue,
            present_image: 0,
            acquire_sem,
            render_sem,
            render_fence,
            start: time,
            frame_start: time,
        }
    }

    pub(crate) fn cur_framebuffer(&self) -> vk::Framebuffer {
        self.res.framebuffers.framebuffers[self.present_image as usize]
    }

    pub(crate) unsafe fn acquire_next_image(&mut self) {
        let swapchain = &self.res.swapchain;
        let dt = &*self.dt;
        dt.acquire_next_image_khr(
            swapchain.inner,            //swapchain
            u64::max_value(),           //timeout
            self.acquire_sem,           //semaphore
            vk::null(),                 //fence
            &mut self.present_image,    //pImageIndex
        );
        self.frame_start = Instant::now();
    }

    pub(crate) unsafe fn wait_for_render(&mut self) {
        let fences = [self.render_fence];
        let dt = &*self.dt;
        dt.wait_for_fences(
            fences.len() as _,  //fenceCount
            fences.as_ptr(),    //pFences
            vk::FALSE,          //waitAll
            u64::max_value(),   //timeout
        );
        let fences = [self.render_fence];
        self.dt.reset_fences(fences.len() as _, fences.as_ptr());
    }

    pub(crate) unsafe fn present(&mut self) {
        let wait_sems = [self.render_sem];
        let swapchains = [self.res.swapchain.inner];
        let present_info = vk::PresentInfoKHR {
            wait_semaphore_count: wait_sems.len() as _,
            p_wait_semaphores: wait_sems.as_ptr(),
            swapchain_count: swapchains.len() as _,
            p_swapchains: swapchains.as_ptr(),
            p_image_indices: &self.present_image,
            ..Default::default()
        };
        self.gfx_queue.present(&present_info).check().unwrap();
    }
}
