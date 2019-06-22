use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::Arc;

use demos::{
    Device, Instance, InstanceConfig, ObjectTracker, Surface, Swapchain, c_str,
    include_shader,
};

const TITLE_BASE: &'static str = "Triangle demo\0";

fn make_title(fps: f32) -> CString {
    let title_base = &TITLE_BASE[..TITLE_BASE.len() - 1];
    let title = format!("{} | {:.2} fps", title_base, fps);
    unsafe { CString::from_vec_unchecked(title.into()) }
}

fn app_title() -> *const c_char {
    TITLE_BASE.as_ptr() as _
}

const VERT_SHADER_SRC: &'static [u8] = include_shader!("triangle_vert.spv");
const FRAG_SHADER_SRC: &'static [u8] = include_shader!("triangle_frag.spv");

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Timestamps {
    pub old: u64,
    pub new: u64,
}

impl Timestamps {
    pub fn to_ns(self, device: &Device) -> f32 {
        let timestamp_period = device.props.limits.timestamp_period;
        ((self.new - self.old) as f64 * timestamp_period as f64) as f32
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct FrameTimer {
    pub device: Arc<Device>,
    pub query_pool: vk::QueryPool,
}

impl FrameTimer {
    pub unsafe fn new(objs: &mut ObjectTracker) -> Self {
        let create_info = vk::QueryPoolCreateInfo {
            query_type: vk::QueryType::TIMESTAMP,
            query_count: 2,
            ..Default::default()
        };
        let query_pool = objs.create_query_pool(&create_info);
        FrameTimer {
            device: Arc::clone(&objs.device),
            query_pool,
        }
    }

    pub unsafe fn get_query_results(&self) -> Timestamps {
        let mut ts: Timestamps = Default::default();
        let data_size = std::mem::size_of::<Timestamps>();
        let stride = std::mem::size_of::<u64>();
        self.device.table.get_query_pool_results(
            self.query_pool,                // queryPool
            0,                              // firstQuery
            2,                              // queryCount
            data_size,                      // dataSize
            &mut ts as *mut _ as _,         // pData
            stride as _,                    // stride
            vk::QueryResultFlags::_64_BIT,  // flags
        ).check_success().unwrap();
        ts
    }

    pub unsafe fn start(&self, cb: vk::CommandBuffer) {
        self.device.table.cmd_reset_query_pool(cb, self.query_pool, 0, 2);
        self.device.table.cmd_write_timestamp(
            cb,
            vk::PipelineStageFlags::TOP_OF_PIPE_BIT,
            self.query_pool,
            0,
        );
    }

    pub unsafe fn end(&self, cb: vk::CommandBuffer) {
        self.device.table.cmd_write_timestamp(
            cb,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE_BIT,
            self.query_pool,
            1,
        );
    }
}

#[derive(Clone, Copy, Debug)]
struct Framebuffer {
    image: vk::Image,
    view: vk::ImageView,
    inner: vk::Framebuffer,
}

#[derive(Debug)]
struct RenderPath {
    swapchain: Arc<Swapchain>,
    objs: Box<ObjectTracker>,
    framebuffers: Vec<Framebuffer>,
    render_pass: vk::RenderPass,
    pipeline: vk::Pipeline,
}

impl RenderPath {
    unsafe fn new(swapchain: Arc<Swapchain>) -> RenderPath {
        init_render_path(swapchain)
    }
}

unsafe fn init_render_path(swapchain: Arc<Swapchain>) -> RenderPath {
    let mut objs = Box::new(ObjectTracker::new(Arc::clone(&swapchain.device)));

    let attachments = [vk::AttachmentDescription {
        format: swapchain.format,
        samples: vk::SampleCountFlags::_1_BIT,
        load_op: vk::AttachmentLoadOp::DONT_CARE,
        store_op: vk::AttachmentStoreOp::STORE,
        initial_layout: vk::ImageLayout::UNDEFINED,
        final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
        ..Default::default()
    }];
    let subpass_attachment_refs = [vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    }];
    let subpasses = [vk::SubpassDescription {
        pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
        color_attachment_count: subpass_attachment_refs.len() as _,
        p_color_attachments: subpass_attachment_refs.as_ptr(),
        ..Default::default()
    }];
    let create_info = vk::RenderPassCreateInfo {
        attachment_count: attachments.len() as _,
        p_attachments: attachments.as_ptr(),
        subpass_count: subpasses.len() as _,
        p_subpasses: subpasses.as_ptr(),
        ..Default::default()
    };
    let render_pass = objs.create_render_pass(&create_info);

    let create_info = Default::default();
    let layout = objs.create_pipeline_layout(&create_info);

    let vert_shader = objs.create_shader(VERT_SHADER_SRC);
    let frag_shader = objs.create_shader(FRAG_SHADER_SRC);

    let p_name = c_str!("main");
    let stages = [
        vk::PipelineShaderStageCreateInfo {
            stage: vk::ShaderStageFlags::VERTEX_BIT,
            module: vert_shader,
            p_name,
            ..Default::default()
        },
        vk::PipelineShaderStageCreateInfo {
            stage: vk::ShaderStageFlags::FRAGMENT_BIT,
            module: frag_shader,
            p_name,
            ..Default::default()
        },
    ];
    let vertex_input_state = Default::default();
    let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo {
        topology: vk::PrimitiveTopology::TRIANGLE_STRIP,
        ..Default::default()
    };
    let viewports = [vk::Viewport {
        x: 0.0,
        y: 0.0,
        width: swapchain.extent.width as _,
        height: swapchain.extent.height as _,
        min_depth: 0.0,
        max_depth: 1.0,
    }];
    let scissors = [swapchain.rectangle()];
    let viewport_state = vk::PipelineViewportStateCreateInfo {
        viewport_count: viewports.len() as _,
        p_viewports: viewports.as_ptr(),
        scissor_count: scissors.len() as _,
        p_scissors: scissors.as_ptr(),
        ..Default::default()
    };
    let rasterization_state = vk::PipelineRasterizationStateCreateInfo {
        polygon_mode: vk::PolygonMode::FILL,
        line_width: 1.0,
        ..Default::default()
    };
    let multisample_state = vk::PipelineMultisampleStateCreateInfo {
        rasterization_samples: vk::SampleCountFlags::_1_BIT,
        ..Default::default()
    };
    let color_blend_attachments = [vk::PipelineColorBlendAttachmentState {
        color_write_mask: vk::ColorComponentFlags::R_BIT
            | vk::ColorComponentFlags::G_BIT
            | vk::ColorComponentFlags::B_BIT
            | vk::ColorComponentFlags::A_BIT,
        ..Default::default()
    }];
    let color_blend_state = vk::PipelineColorBlendStateCreateInfo {
        attachment_count: color_blend_attachments.len() as _,
        p_attachments: color_blend_attachments.as_ptr(),
        ..Default::default()
    };
    let create_info = vk::GraphicsPipelineCreateInfo {
        stage_count: stages.len() as _,
        p_stages: stages.as_ptr(),
        p_vertex_input_state: &vertex_input_state as _,
        p_input_assembly_state: &input_assembly_state as _,
        p_viewport_state: &viewport_state as _,
        p_rasterization_state: &rasterization_state as _,
        p_multisample_state: &multisample_state as _,
        p_color_blend_state: &color_blend_state as _,
        layout,
        render_pass,
        subpass: 0,
        ..Default::default()
    };
    let pipeline = objs.create_graphics_pipeline(&create_info);

    let framebuffers: Vec<_> = swapchain.images.iter().map(|&image| {
        let view =
            demos::create_swapchain_image_view(&mut objs, &swapchain, image);
        let inner = demos::create_swapchain_framebuffer
            (&mut objs, &swapchain, render_pass, view);
        Framebuffer {
            image,
            view,
            inner,
        }
    }).collect();

    RenderPath {
        swapchain,
        objs,
        framebuffers,
        render_pass,
        pipeline,
    }
}

#[derive(Debug)]
struct FrameState {
    dt: Arc<vkl::DeviceTable>,
    path: Arc<RenderPath>,
    objs: Box<ObjectTracker>,
    cmd_pool: vk::CommandPool,
    timer: FrameTimer,
    done_sem: vk::Semaphore,
    done_fence: vk::Fence,
    framebuf_idx: u32,
    cmds: vk::CommandBuffer,
}

#[derive(Clone, Copy, Debug, Default)]
struct FrameLog {
    time_ns: f32,
}

impl FrameState {
    unsafe fn new(path: Arc<RenderPath>) -> Self {
        let device = &path.swapchain.device;
        let mut objs = Box::new(ObjectTracker::new(Arc::clone(device)));

        let create_info = vk::CommandPoolCreateInfo {
            flags: vk::CommandPoolCreateFlags::TRANSIENT_BIT,
            queue_family_index: 0,
            ..Default::default()
        };
        let cmd_pool = objs.create_command_pool(&create_info);

        let alloc_info = vk::CommandBufferAllocateInfo {
            command_pool: cmd_pool,
            command_buffer_count: 1,
            ..Default::default()
        };
        let mut cmds = vk::null();
        objs.alloc_command_buffers(
            &alloc_info,
            std::slice::from_mut(&mut cmds),
        );

        let timer = FrameTimer::new(&mut objs);

        let done_sem = objs.create_semaphore();
        let done_fence = objs.create_fence(true);

        FrameState {
            dt: Arc::clone(&device.table),
            path,
            objs,
            cmd_pool,
            timer,
            done_sem,
            done_fence,
            framebuf_idx: 0,
            cmds,
        }
    }

    fn framebuffer(&self) -> &Framebuffer {
        &self.path.framebuffers[self.framebuf_idx as usize]
    }

    unsafe fn record(&mut self) {
        let dt = &self.dt;

        dt.reset_command_pool(self.cmd_pool, Default::default());

        // Record commands
        let cb = self.cmds;
        let begin_info = Default::default();
        dt.begin_command_buffer(cb, &begin_info as _);

        self.timer.start(cb);

        let framebuffer = self.framebuffer().inner;
        let render_area = self.path.swapchain.rectangle();
        let begin_info = vk::RenderPassBeginInfo {
            render_pass: self.path.render_pass,
            framebuffer,
            render_area,
            ..Default::default()
        };
        dt.cmd_begin_render_pass(cb, &begin_info as _, Default::default());

        dt.cmd_bind_pipeline(
            cb,
            vk::PipelineBindPoint::GRAPHICS,
            self.path.pipeline,
        );
        dt.cmd_draw(cb, 4, 1, 0, 0);

        dt.cmd_end_render_pass(cb);

        self.timer.end(cb);

        dt.end_command_buffer(cb);
    }

    unsafe fn submit(&mut self, queue: vk::Queue, wait_sem: vk::Semaphore) {
        let wait_sems = std::slice::from_ref(&wait_sem);
        let wait_masks = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT_BIT];
        let cmds = std::slice::from_ref(&self.cmds);
        let sig_sems = std::slice::from_ref(&self.done_sem);
        let submit_info = vk::SubmitInfo {
            wait_semaphore_count: wait_sems.len() as _,
            p_wait_semaphores: wait_sems.as_ptr(),
            p_wait_dst_stage_mask: wait_masks.as_ptr(),
            command_buffer_count: cmds.len() as _,
            p_command_buffers: cmds.as_ptr(),
            signal_semaphore_count: sig_sems.len() as _,
            p_signal_semaphores: sig_sems.as_ptr(),
            ..Default::default()
        };
        self.dt.queue_submit(queue, 1, &submit_info as _, self.done_fence)
            .check().unwrap();
    }

    unsafe fn wait_until_done(&mut self) {
        self.dt.wait_for_fences
            (1, &self.done_fence as _, vk::FALSE, u64::max_value())
            .check_success().unwrap();
        self.dt.reset_fences(1, &self.done_fence as _).check().unwrap();
    }

    unsafe fn collect_log(&mut self) -> FrameLog {
        // Gather statistics after rendering
        let ts = self.timer.get_query_results();
        let time_ns = ts.to_ns(&self.path.swapchain.device);
        FrameLog { time_ns }
    }
}

const FRAME_HISTORY_SIZE: usize = 64;
const NUM_FRAMES: usize = 2;

struct RenderState {
    device: Arc<Device>,
    swapchain: Arc<Swapchain>,
    objs: Box<ObjectTracker>,
    frames: Box<[FrameState; NUM_FRAMES]>,
    frame_counter: u64,
    history: Box<[FrameLog; FRAME_HISTORY_SIZE]>,
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
    unsafe fn new(swapchain: Arc<Swapchain>) -> Self {
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

    unsafe fn acquire_framebuffer(
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

    unsafe fn record_log(&mut self, log: FrameLog) {
        assert!(self.frame_counter >= 2);
        let idx = (self.frame_counter - 2) % FRAME_HISTORY_SIZE as u64;
        self.history[idx as usize] = log;
    }

    // Waits for an old frame to finish before reusing its resources to
    // prepare the next frame.
    //
    // Waiting should only occur when we are rendering at >60fps.
    unsafe fn wait_for_next_frame(&mut self, present_sem: vk::Semaphore) {
        self.frame_counter += 1;

        cur_frame_mut!(self).wait_until_done();
        if self.frame_counter > 2 {
            let log = cur_frame_mut!(self).collect_log();
            self.record_log(log);
        }
        cur_frame_mut!(self).framebuf_idx =
            self.acquire_framebuffer(present_sem);
    }

    unsafe fn render(&mut self, queue: vk::Queue, wait_sem: vk::Semaphore) {
        let frame = cur_frame_mut!(self);
        frame.record();
        frame.submit(queue, wait_sem);
    }

    unsafe fn present(
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

    fn history_full(&self) -> bool {
        self.frame_counter % FRAME_HISTORY_SIZE as u64 == 0
    }

    unsafe fn compute_fps(&self) -> f32 {
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

unsafe fn init_video() -> Arc<Swapchain> {
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

    let pdev = demos::device_for_surface(&surface).unwrap();

    let config = Default::default();
    let device = Arc::new(Device::new(instance, pdev, config).unwrap());

    Arc::new(Swapchain::new(surface, device).unwrap())
}

fn main() {
    unsafe { unsafe_main() }
}

unsafe fn unsafe_main() {
    let swapchain = init_video();

    let window = Arc::clone(&swapchain.surface.window);
    let device = Arc::clone(&swapchain.device);
    let queue = device.get_queue(0, 0);

    let mut state = RenderState::new(swapchain);

    let mut objs = Box::new(ObjectTracker::new(Arc::clone(&device)));

    // ???: Can this be doubly used when graphics/present are split?
    let present_sem = objs.create_semaphore();

    loop {
        state.wait_for_next_frame(present_sem);

        // Update FPS counter
        if state.history_full() {
            let title = make_title(state.compute_fps());
            window.set_title(title.as_ptr());
        }

        state.render(queue, present_sem);
        state.present(queue);

        window.sys().poll_events();
        if window.should_close() { break; }
    }
}
