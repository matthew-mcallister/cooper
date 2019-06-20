//! Displays a triangle inside a window.
use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::Arc;

use demos::c_str;

const TITLE_BASE: &'static str = "Triangle demo\0";

fn make_title(fps: f32) -> CString {
    let title_base = &TITLE_BASE[..TITLE_BASE.len() - 1];
    let title = format!("{} | {:.2} fps", title_base, fps);
    unsafe { CString::from_vec_unchecked(title.into()) }
}

fn app_title() -> *const c_char {
    TITLE_BASE.as_ptr() as _
}

const VERT_SHADER_SRC: &'static [u8] =
    demos::include_shader!("triangle_vert.spv");
const FRAG_SHADER_SRC: &'static [u8] =
    demos::include_shader!("triangle_frag.spv");

const FRAME_HISTORY_SIZE: usize = 64;

#[derive(Clone, Copy, Debug)]
struct Framebuffer {
    image: vk::Image,
    view: vk::ImageView,
    framebuffer: vk::Framebuffer,
}

#[derive(Debug)]
struct Frame {
    framebuffer: u32,
    commands: vk::CommandBuffer,
    fence: vk::Fence,
    timer: demos::DeviceTimer,
}

#[derive(Clone, Copy, Debug, Default)]
struct FrameLog {
    time_ns: f32,
}

const NUM_FRAMES: usize = 2;

struct RenderState {
    device: Arc<demos::Device>,
    swapchain: Arc<demos::Swapchain>,
    framebuffers: Vec<Framebuffer>,
    history: [FrameLog; FRAME_HISTORY_SIZE],
    frames: [Frame; NUM_FRAMES],
    frame_counter: u64,
}

impl std::fmt::Debug for RenderState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "RenderState {{ ... }}")
    }
}

impl RenderState {
    unsafe fn new(
        gfx: &mut demos::GfxObjects,
        swapchain: &Arc<demos::Swapchain>,
        render_pass: vk::RenderPass,
    ) -> Self {
        let framebuffers: Vec<_> = swapchain.images.iter().map(|&image| {
            let view =
                demos::create_swapchain_image_view(gfx, swapchain, image);
            let framebuffer = demos::create_swapchain_framebuffer
                (gfx, swapchain, render_pass, view);
            Framebuffer {
                image,
                view,
                framebuffer,
            }
        }).collect();

        // TODO: Command pools per frame (and per thread)
        // You can reset the whole command pool that way
        let create_info = vk::CommandPoolCreateInfo {
            flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER_BIT,
            queue_family_index: 0,
            ..Default::default()
        };
        let command_pool = gfx.create_command_pool(&create_info);

        let alloc_info = vk::CommandBufferAllocateInfo {
            command_pool,
            command_buffer_count: NUM_FRAMES as _,
            ..Default::default()
        };
        let mut command_buffers = vec![vk::null(); NUM_FRAMES];
        gfx.alloc_command_buffers(&alloc_info, &mut command_buffers[..]);

        let mut frames = Vec::with_capacity(NUM_FRAMES);
        for commands in command_buffers {
            let timer = demos::DeviceTimer::new(gfx);
            let fence = gfx.create_fence(true);
            frames.push(Frame {
                framebuffer: 0,
                commands,
                fence,
                timer,
            });
        }

        RenderState {
            device: Arc::clone(&gfx.device),
            swapchain: Arc::clone(swapchain),
            framebuffers,
            history: [Default::default(); FRAME_HISTORY_SIZE],
            frames: [frames.pop().unwrap(), frames.pop().unwrap()],
            frame_counter: 0,
        }
    }

    fn cur_frame(&self) -> &Frame {
        &self.frames[(self.frame_counter % 2) as usize]
    }

    fn cur_frame_mut(&mut self) -> &mut Frame {
        &mut self.frames[(self.frame_counter % 2) as usize]
    }

    // Waits for an old frame to finish to reuse its resources.
    //
    // Waiting should only occur when we are rendering at >60fps.
    unsafe fn wait_for_next_frame(&mut self, present_sem: vk::Semaphore) {
        self.frame_counter += 1;

        let frame = &*(self.cur_frame_mut() as *mut Frame);
        let dt = &self.device.table;

        dt.wait_for_fences
            (1, &frame.fence as _, vk::FALSE, u64::max_value())
            .check_success().unwrap();
        dt.reset_fences(1, &frame.fence as _).check().unwrap();

        // Gather statistics from old frame
        if self.frame_counter > 2 {
            let ts = frame.timer.get_query_results().unwrap();
            let time_ns = self.device.timestamps_to_ns(ts);
            self.history[(self.frame_counter % 60) as usize] = FrameLog {
                time_ns,
            };
        }

        // Wait for framebuffers to come available
        let mut idx = 0;
        self.device.table.acquire_next_image_khr(
            self.swapchain.inner,   // swapchain
            u64::max_value(),       // timeout
            present_sem,            // semaphore
            vk::null(),             // fence
            &mut idx as _,          // pImageIndex
        ).check_success().unwrap();
        self.cur_frame_mut().framebuffer = idx;
    }

    unsafe fn present_frame(
        &mut self,
        queue: vk::Queue,
        wait_sem: vk::Semaphore,
    ) {
        let wait_sems = [wait_sem];
        let swapchains = std::slice::from_ref(&self.swapchain.inner);
        let indices = [self.cur_frame().framebuffer];
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

fn main() {
    unsafe { unsafe_main() }
}

unsafe fn unsafe_main() {
    let config = demos::InstanceConfig {
        app_info: vk::ApplicationInfo {
            p_application_name: app_title(),
            application_version: vk::make_version!(0, 1, 0),
            api_version: vk::API_VERSION_1_1,
            ..Default::default()
        },
        ..Default::default()
    };
    let instance = demos::Instance::new(config).unwrap();

    let title = CString::from_vec_unchecked(make_title(0.0).into());
    let dims = (1280, 720).into();
    let config = window::Config {
        title: title.as_ptr(),
        dims,
        hints: Default::default(),
    };
    let surface = demos::Surface::new(&instance, config).unwrap();

    let window = Arc::clone(&surface.window);

    let pdev = demos::device_for_surface(&surface).unwrap();

    let config = Default::default();
    let device = demos::Device::new(&instance, pdev, config).unwrap();
    let queue = device.get_queue(0, 0);
    let swapchain = demos::Swapchain::new(&surface, &device).unwrap();

    let mut gfx = demos::GfxObjects::new(&device);

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
    let render_pass = gfx.create_render_pass(&create_info);

    let create_info = Default::default();
    let layout = gfx.create_pipeline_layout(&create_info);

    let vert_shader = gfx.create_shader(VERT_SHADER_SRC);
    let frag_shader = gfx.create_shader(FRAG_SHADER_SRC);

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
    let render_area = vk::Rect2D::new(
        vk::Offset2D::new(0, 0),
        swapchain.extent,
    );
    let scissors = std::slice::from_ref(&render_area);
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
    let pipeline = gfx.create_graphics_pipeline(&create_info);

    let mut render_state = RenderState::new(&mut gfx, &swapchain, render_pass);

    // ???: Can these be doubly used when graphics/present are split?
    let present_sem = gfx.create_semaphore();
    let graphics_sem = gfx.create_semaphore();

    loop {
        let dt = &device.table;

        render_state.wait_for_next_frame(present_sem);

        // Record for new frame
        let frame = render_state.cur_frame();

        let cb = frame.commands;
        let begin_info = Default::default();
        dt.begin_command_buffer(cb, &begin_info as _);

        frame.timer.start(cb);

        // TODO: Three fields named `framebuffer` is too many
        let framebuffer = render_state.framebuffers[frame.framebuffer as usize]
            .framebuffer;
        let begin_info = vk::RenderPassBeginInfo {
            render_pass,
            framebuffer,
            render_area,
            ..Default::default()
        };
        dt.cmd_begin_render_pass(cb, &begin_info as _, Default::default());

        dt.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline);
        dt.cmd_draw(cb, 4, 1, 0, 0);

        dt.cmd_end_render_pass(cb);

        frame.timer.end(cb);

        dt.end_command_buffer(cb);

        // Update FPS counter
        if render_state.frame_counter % FRAME_HISTORY_SIZE as u64 == 0 {
            let title = make_title(render_state.compute_fps());
            window.set_title(title.as_ptr());
        }

        let wait_sems = std::slice::from_ref(&present_sem);
        let stage_masks =
            [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT_BIT];
        let command_buffers = std::slice::from_ref(&frame.commands);
        let sig_sems = std::slice::from_ref(&graphics_sem);
        let submit_info = vk::SubmitInfo {
            wait_semaphore_count: wait_sems.len() as _,
            p_wait_semaphores: wait_sems.as_ptr(),
            p_wait_dst_stage_mask: stage_masks.as_ptr(),
            command_buffer_count: command_buffers.len() as _,
            p_command_buffers: command_buffers.as_ptr(),
            signal_semaphore_count: sig_sems.len() as _,
            p_signal_semaphores: sig_sems.as_ptr(),
            ..Default::default()
        };
        dt.queue_submit(queue, 1, &submit_info as _, frame.fence)
            .check().unwrap();

        render_state.present_frame(queue, graphics_sem);

        window.sys().poll_events();
        if window.should_close() { break; }
    }
}
