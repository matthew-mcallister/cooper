//! Displays a triangle inside a window.
use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::Arc;

use demos::{assert_success, c_str};

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

const FRAME_HISTORY_SIZE: usize = 60;

#[derive(Clone, Copy, Debug, Default)]
struct SwapchainFrame {
    run_yet: bool,
    image: vk::Image,
    view: vk::ImageView,
    framebuffer: vk::Framebuffer,
    commands: vk::CommandBuffer,
    fence: vk::Fence,
    query_pool: vk::QueryPool,
}

unsafe fn create_swapchain_frames(
    gfx: &mut demos::GfxState,
    swapchain: &demos::Swapchain,
    render_pass: vk::RenderPass,
) -> Vec<SwapchainFrame> {
    let num_frames = swapchain.images.len();

    let create_info = Default::default();
    let command_pool = gfx.create_command_pool(&create_info);

    let alloc_info = vk::CommandBufferAllocateInfo {
        command_pool,
        command_buffer_count: num_frames as _,
        ..Default::default()
    };
    let mut command_buffers = vec![vk::null(); num_frames];
    gfx.alloc_command_buffers(&alloc_info, &mut command_buffers[..]);

    let mut frames = Vec::with_capacity(num_frames);
    for (idx, &image) in swapchain.images.iter().enumerate() {
        let view = demos::create_swapchain_image_view(gfx, swapchain, image);
        let framebuffer = demos::create_swapchain_framebuffer
            (gfx, swapchain, render_pass, view);

        let create_info = vk::QueryPoolCreateInfo {
            query_type: vk::QueryType::TIMESTAMP,
            query_count: 2,
            ..Default::default()
        };
        let query_pool = gfx.create_query_pool(&create_info);

        frames.push(SwapchainFrame {
            run_yet: false,
            image,
            view,
            framebuffer,
            commands: command_buffers[idx],
            fence: gfx.create_fence(true),
            query_pool,
        });
    }

    frames
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
    let config = window::Config {
        title: title.as_ptr(),
        dims: (1280, 720).into(),
    };
    let surface = demos::Surface::new(&instance, config).unwrap();

    let pdev = demos::device_for_surface(&surface).unwrap();
    let device_props = instance.get_properties(pdev);

    let timestamp_period = device_props.limits.timestamp_period;

    let config = Default::default();
    let device = demos::Device::new(&instance, pdev, config).unwrap();
    let queue = device.get_queue(0, 0);
    let swapchain = demos::Swapchain::new(&surface, &device).unwrap();

    let mut gfx = demos::GfxState::new(&device);

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

    let mut frames = create_swapchain_frames(&mut gfx, &swapchain, render_pass);
    for frame in frames.iter() {
        let dt = &gfx.device.table;

        let cb = frame.commands;
        let begin_info = Default::default();
        dt.begin_command_buffer(cb, &begin_info as _);

        dt.cmd_reset_query_pool(cb, frame.query_pool, 0, 2);
        dt.cmd_write_timestamp(
            cb,
            vk::PipelineStageFlags::TOP_OF_PIPE_BIT,
            frame.query_pool,
            0,
        );

        let begin_info = vk::RenderPassBeginInfo {
            render_pass,
            framebuffer: frame.framebuffer,
            render_area,
            ..Default::default()
        };
        dt.cmd_begin_render_pass(cb, &begin_info as _, Default::default());

        dt.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline);
        dt.cmd_draw(cb, 4, 1, 0, 0);

        dt.cmd_end_render_pass(cb);

        dt.cmd_write_timestamp(
            cb,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE_BIT,
            frame.query_pool,
            1,
        );

        dt.end_command_buffer(cb);
    }

    // ???: Can these be doubly used when graphics/present are split?
    let present_sem = gfx.create_semaphore();
    let graphics_sem = gfx.create_semaphore();

    let mut frame_num = 0;
    let mut frame_times_ns = vec![0f32; FRAME_HISTORY_SIZE];

    let window = Arc::clone(&surface.window);
    loop {
        let dt = &gfx.device.table;

        let mut idx: u32 = 0;
        assert_success!(dt.acquire_next_image_khr(
            swapchain.inner,    // swapchain
            u64::max_value(),   // timeout
            present_sem,        // semaphore
            vk::null(),         // fence
            &mut idx as _,      // pImageIndex
        ));

        let frame = &mut frames[idx as usize];

        // Wait for old frame to finish
        assert_success!(dt.wait_for_fences
            (1, &frame.fence as _, vk::FALSE, u64::max_value()));
        dt.reset_fences(1, &frame.fence as _).check().unwrap();

        // Retrieve frame time
        if frame.run_yet {
            #[repr(C)]
            #[derive(Clone, Copy, Debug, Default)]
            struct Timestamps {
                old: u64,
                new: u64,
            }
            let mut timestamps: Timestamps = Default::default();
            let data_size = std::mem::size_of::<Timestamps>();
            let stride = std::mem::size_of::<u64>();
            assert_success!(dt.get_query_pool_results(
                frame.query_pool,                   // queryPool
                0,                                  // firstQuery
                2,                                  // queryCount
                data_size,                          // dataSize
                &mut timestamps as *mut _ as _,     // pData
                stride as _,                        // stride
                vk::QueryResultFlags::_64_BIT,      // flags
            ));

            // Update framerate
            let timestamp_diff = (timestamps.new - timestamps.old) as f32;
            let frame_ns = timestamp_diff * timestamp_period;
            frame_times_ns[frame_num % FRAME_HISTORY_SIZE] = frame_ns;

            frame_num += 1;
            if frame_num % FRAME_HISTORY_SIZE == 0 {
                let total_time_ns: f32 = frame_times_ns.iter().cloned().sum();
                let total_time = total_time_ns * 1e-9;
                let fps = FRAME_HISTORY_SIZE as f32 / total_time;

                let title = make_title(fps);
                window.set_title(title.as_ptr());
            }
        } else {
            frame.run_yet = true;
        }

        // Begin new frame
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

        let wait_sems = sig_sems;
        let swapchains = std::slice::from_ref(&swapchain.inner);
        let indices = std::slice::from_ref(&idx);
        let present_info = vk::PresentInfoKHR {
            wait_semaphore_count: wait_sems.len() as _,
            p_wait_semaphores: wait_sems.as_ptr(),
            swapchain_count: swapchains.len() as _,
            p_swapchains: swapchains.as_ptr(),
            p_image_indices: indices.as_ptr(),
            ..Default::default()
        };
        assert_success!(dt.queue_present_khr(queue, &present_info as _));

        window.sys().poll_events();
        if window.should_close() { break; }
    }
}
