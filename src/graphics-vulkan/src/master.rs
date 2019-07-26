use std::fs;
use std::io;
use std::os::raw::c_char;
use std::sync::Arc;

use crate::*;

pub unsafe fn init_video(
    app_title: *const c_char,
    window: Arc<window::Window>,
) -> RenderState {
    assert!(window.sys().vulkan_supported());
    let config = InstanceConfig {
        app_info: vk::ApplicationInfo {
            p_application_name: app_title,
            application_version: vk::make_version!(0, 1, 0),
            api_version: vk::API_VERSION_1_1,
            p_engine_name: c_str!("cooper"),
            engine_version: vk::make_version!(0, 1, 0),
            ..Default::default()
        },
        ..Default::default()
    };
    let instance = Arc::new(Instance::new(config).unwrap());
    let surface =
        Arc::new(Surface::new(Arc::clone(&instance), window).unwrap());
    let pdev = device_for_surface(&surface).unwrap();
    let device = Arc::new(Device::new(instance, pdev).unwrap());
    let swapchain = Arc::new(Swapchain::new(surface, device).unwrap());
    RenderState::new(swapchain)
}

const FRAME_HISTORY_SIZE: usize = 64;

#[derive(Debug)]
pub struct InitResources {
    pub objs: ObjectTracker,
    pub mapped_mem: MemoryPool,
}

pub struct RenderState {
    pub device: Arc<Device>,
    pub swapchain: Arc<Swapchain>,
    pub gfx_queue: vk::Queue,
    pub gfx_queue_family: u32,
    pub res: Box<InitResources>,
    pub present_sem: vk::Semaphore,
    pub textures: Box<TextureManager>,
    pub frames: Box<[FrameState; 2]>,
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
    res
    frames
    frame_counter
    (history: "[...]")
});

// These are written as macros to appease the borrow checker (without
// casting to a pointer)
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

        let gfx_queue_family = 0;
        let gfx_queue = device.get_queue(0, 0);

        let type_index = find_memory_type(&device, visible_coherent_memory())
            .unwrap();
        let create_info = MemoryPoolCreateInfo {
            type_index,
            mapped: true,
            base_size: 0x100_0000,
        };
        let mapped_mem = MemoryPool::new(Arc::clone(&device), create_info);

        let objs = ObjectTracker::new(Arc::clone(&device));
        let mut res = Box::new(InitResources { objs, mapped_mem });

        let present_sem = res.objs.create_semaphore();

        let path = Arc::new(RenderPath::new(Arc::clone(&swapchain), &mut res));
        let textures = Box::new(TextureManager::new
            (&mut res, Arc::clone(&path), gfx_queue_family));
        let frames = Box::new(FrameState::new_pair(path, &mut res));

        RenderState {
            device,
            swapchain,
            gfx_queue,
            gfx_queue_family,
            res,
            present_sem,
            textures,
            frames,
            frame_counter: 0,
            history: Box::new([Default::default(); FRAME_HISTORY_SIZE]),
        }
    }

    pub unsafe fn load_textures(&mut self) {
        // TODO: Load asynchronously
        // TODO: Is PNG decoding a bottleneck here? i.e. would this go
        // faster if it were parallelized?
        for _ in 0..256 {
            let src = fs::File::open("/tmp/test_pattern.png").unwrap();
            let src = io::BufReader::new(src);
            self.textures.load_png(src).unwrap();
        }
        self.textures.flush();
    }

    pub unsafe fn acquire_framebuffer(&mut self) -> u32 {
        let mut idx = 0;
        self.device.table.acquire_next_image_khr(
            self.swapchain.inner,   // swapchain
            u64::max_value(),       // timeout
            self.present_sem,       // semaphore
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
    // Waiting should only occur when rendering at >60fps.
    pub unsafe fn wait_for_next_frame(&mut self) {
        self.frame_counter += 1;

        cur_frame_mut!(self).wait_until_done();
        if self.frame_counter > 2 {
            let log = cur_frame_mut!(self).collect_log();
            self.record_log(log);
        }
        cur_frame_mut!(self).framebuf_idx = self.acquire_framebuffer();
    }

    pub fn set_sprite_count(&mut self, count: u32) {
        cur_frame_mut!(self).sprite_count = count;
    }

    pub fn sprites(&self) -> *mut [Sprite] {
        let frame = cur_frame!(self);
        let count = frame.sprite_count;
        unsafe { &mut (*frame.sprite_buf.data)[..count as _] as _ }
    }

    pub unsafe fn render(&mut self) {
        let frame = cur_frame_mut!(self);
        frame.record();
        frame.submit(self.gfx_queue, self.present_sem);
    }

    pub unsafe fn present(&mut self) {
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
        self.device.table.queue_present_khr(self.gfx_queue, &present_info as _)
            .check_success().unwrap();
    }

    pub fn history_full(&self) -> bool {
        self.frame_counter % FRAME_HISTORY_SIZE as u64 == 0
    }

    // TODO: This is not really the renderer's business
    pub unsafe fn compute_fps(&self) -> f32 {
        let total_time_ns: f32 = self.history.iter()
            .map(|frame| frame.time_ns)
            .sum();
        if total_time_ns < 1.0 {
            // Avoid divide-by-zero edge cases
            return 0.0;
        }
        let total_time = total_time_ns * 1e-9;
        FRAME_HISTORY_SIZE as f32 / total_time
    }
}
