use std::sync::Arc;

use crate::*;

const FRAME_HISTORY_SIZE: usize = 64;
const NUM_FRAMES: usize = 2;

pub unsafe fn init_video() -> Arc<Swapchain> {
    let config = InstanceConfig {
        app_info: vk::ApplicationInfo {
            p_application_name: app_title(),
            application_version: vk::make_version!(0, 1, 0),
            api_version: vk::API_VERSION_1_1,
            ..Default::default()
        },
        ..Default::default()
    };
    let instance = Arc::new(Instance::new(config).unwrap());

    let title = CString::from_vec_unchecked(make_title(0.0).into());
    let dims = (1280, 720).into();
    let config = window::Config {
        title: title.as_ptr(),
        dims,
        hints: Default::default(),
    };
    let surface =
        Arc::new(Surface::new(Arc::clone(&instance), config).unwrap());

    let pdev = device_for_surface(&surface).unwrap();

    let config = Default::default();
    let device = Arc::new(Device::new(instance, pdev, config).unwrap());

    Arc::new(Swapchain::new(surface, device).unwrap())
}

pub struct RenderState {
    pub device: Arc<Device>,
    pub swapchain: Arc<Swapchain>,
    pub objs: Box<ObjectTracker>,
    pub frames: Box<[FrameState; NUM_FRAMES]>,
    pub frame_counter: u64,
    pub history: Box<[FrameLog; FRAME_HISTORY_SIZE]>,
}

macro_rules! impl_debug {
    ($struct:ident { $($inner:tt)* }) => {
        impl std::fmt::Debug for $struct {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                let mut f = f.debug_struct(stringify!($struct));
                $(impl_debug!(@inner self, f, $inner);)*
                f.finish()
            }
        }
    };
    (@inner $self:expr, $fmt:expr, $field:ident) => {
        $fmt.field(stringify!($field), &$self.$field);
    };
    (@inner $self:expr, $fmt:expr, ($field:ident: $str:expr)) => {
        $fmt.field(stringify!($field), &$str);
    };
}

impl_debug!(RenderState {
    device
    swapchain
    objs
    frames
    frame_counter
    (history: "[...]")
});

// These are written as macros to appease the borrow checker (without
// resorting to pointer-casting magic)
macro_rules! cur_frame {
    ($self:expr) => {
        &$self.frames[($self.frame_counter % 2) as usize]
    }
}

macro_rules! cur_frame_mut {
    ($self:expr) => {
        &mut $self.frames[($self.frame_counter % 2) as usize]
    }
}

impl RenderState {
    pub unsafe fn new(swapchain: Arc<Swapchain>) -> Self {
        let device = Arc::clone(&swapchain.device);
        let objs = Box::new(ObjectTracker::new(Arc::clone(&device)));

        let path = Arc::new(RenderPath::new(Arc::clone(&swapchain)));

        let frames = Box::new([
            FrameState::new(Arc::clone(&path)),
            FrameState::new(path),
        ]);

        RenderState {
            device,
            swapchain,
            objs,
            frames,
            frame_counter: 0,
            history: Box::new([Default::default(); FRAME_HISTORY_SIZE]),
        }
    }

    pub unsafe fn acquire_framebuffer(
        &mut self,
        present_sem: vk::Semaphore,
    ) -> u32 {
        let mut idx = 0;
        self.device.table.acquire_next_image_khr(
            self.swapchain.inner,   // swapchain
            u64::max_value(),       // timeout
            present_sem,            // semaphore
            vk::null(),             // fence
            &mut idx as _,          // pImageIndex
        ).check_success().unwrap();
        idx
    }

    pub unsafe fn record_log(&mut self, log: FrameLog) {
        assert!(self.frame_counter >= 2);
        let idx = (self.frame_counter - 2) % FRAME_HISTORY_SIZE as u64;
        self.history[idx as usize] = log;
    }

    // Waits for an old frame to finish before reusing its resources to
    // prepare the next frame.
    //
    // Waiting should only occur when we are rendering at >60fps.
    pub unsafe fn wait_for_next_frame(&mut self, present_sem: vk::Semaphore) {
        self.frame_counter += 1;

        cur_frame_mut!(self).wait_until_done();
        if self.frame_counter > 2 {
            let log = cur_frame_mut!(self).collect_log();
            self.record_log(log);
        }
        cur_frame_mut!(self).framebuf_idx =
            self.acquire_framebuffer(present_sem);
    }

    pub unsafe fn render(&mut self, queue: vk::Queue, wait_sem: vk::Semaphore)
    {
        let frame = cur_frame_mut!(self);
        frame.record();
        frame.submit(queue, wait_sem);
    }

    pub unsafe fn present(
        &mut self,
        queue: vk::Queue,
    ) {
        let frame = cur_frame!(self);
        let wait_sems = std::slice::from_ref(&frame.done_sem);
        let swapchains = std::slice::from_ref(&self.swapchain.inner);
        let indices = [frame.framebuf_idx];
        let present_info = vk::PresentInfoKHR {
            wait_semaphore_count: wait_sems.len() as _,
            p_wait_semaphores: wait_sems.as_ptr(),
            swapchain_count: swapchains.len() as _,
            p_swapchains: swapchains.as_ptr(),
            p_image_indices: indices.as_ptr(),
            ..Default::default()
        };
        self.device.table.queue_present_khr(queue, &present_info as _)
            .check_success().unwrap();
    }

    pub fn history_full(&self) -> bool {
        self.frame_counter % FRAME_HISTORY_SIZE as u64 == 0
    }

    pub unsafe fn compute_fps(&self) -> f32 {
        let total_time_ns: f32 = self.history.iter()
            .map(|frame| frame.time_ns)
            .sum();
        if total_time_ns < 1.0 {
            // Avoid divide by zero edge cases
            return 0.0;
        }
        let total_time = total_time_ns * 1e-9;
        FRAME_HISTORY_SIZE as f32 / total_time
    }
}
