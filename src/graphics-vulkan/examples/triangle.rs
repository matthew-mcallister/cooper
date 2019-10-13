use std::ffi::CString;
use std::ptr;
use std::sync::Arc;

use cooper_graphics_vulkan::*;

#[macro_use]
mod common;

use common::*;

unsafe fn init_resources(
    swapchain: Arc<Swapchain>,
    queues: Vec<Vec<Arc<Queue>>>,
) -> AppResources {
    let window = Arc::clone(&swapchain.surface.window);
    let device = Arc::clone(&swapchain.device);

    let set_layouts =
        Arc::new(DescriptorSetLayoutManager::new(Arc::clone(&device)));

    let mut pipe_layouts =
        PipelineLayoutManager::new(Arc::clone(&set_layouts));
    pipe_layouts.create_layout("null".to_owned(), Vec::new());
    let pipe_layouts = Arc::new(pipe_layouts);

    let mut shaders = ShaderManager::new(Arc::clone(&device));
    shaders.create_shader("triangle_vert".to_owned(), ShaderDesc {
        entry: CString::new("main".to_owned()).unwrap(),
        code: include_shader!("triangle_vert.spv").to_vec(),
        set_bindings: Vec::new(),
    });
    shaders.create_shader("triangle_frag".to_owned(), ShaderDesc {
        entry: CString::new("main".to_owned()).unwrap(),
        code: include_shader!("triangle_frag.spv").to_vec(),
        set_bindings: Vec::new(),
    });
    let shaders = Arc::new(shaders);

    let mut render_passes = RenderPassManager::new(Arc::clone(&device));
    let attachments = [vk::AttachmentDescription {
        format: swapchain.format,
        samples: vk::SampleCountFlags::_1_BIT,
        load_op: vk::AttachmentLoadOp::DONT_CARE,
        store_op: vk::AttachmentStoreOp::STORE,
        initial_layout: vk::ImageLayout::UNDEFINED,
        final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
        ..Default::default()
    }];
    let color_attachs = [vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    }];
    let subpasses = [vk::SubpassDescription {
        pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
        color_attachment_count: color_attachs.len() as _,
        p_color_attachments: color_attachs.as_ptr(),
        ..Default::default()
    }];
    let create_info = vk::RenderPassCreateInfo {
        attachment_count: attachments.len() as _,
        p_attachments: attachments.as_ptr(),
        subpass_count: subpasses.len() as _,
        p_subpasses: subpasses.as_ptr(),
        ..Default::default()
    };
    render_passes.create_render_pass(
        "forward".to_owned(),
        &create_info,
        vec!["color".to_owned()],
    );
    let render_passes = Arc::new(render_passes);

    let attachments = Arc::new(AttachmentChain::from_swapchain(&swapchain));
    let framebuffers = Arc::new(render_passes.create_framebuffers(
        "forward".to_owned(),
        vec![attachments],
    ));

    AppResources {
        window,
        swapchain,
        queues,
        set_layouts,
        pipe_layouts,
        shaders,
        render_passes,
        framebuffers,
    }
}

type PipelineDesc = ();

#[derive(Debug)]
struct PipelineFactory {
    res: Arc<AppResources>,
}

impl PipelineFactory {
    fn new(res: Arc<AppResources>) -> Self {
        PipelineFactory {
            res,
        }
    }
}

impl GraphicsPipelineFactory for PipelineFactory {
    type Desc = PipelineDesc;

    unsafe fn create_pipeline(&mut self, _: &Self::Desc) -> GraphicsPipeline {
        let swapchain = &self.res.swapchain;
        let dt = &swapchain.device.table;

        let render_passes = &self.res.render_passes;
        let shaders = &self.res.shaders;
        let pipe_layouts = &self.res.pipe_layouts;

        let vert = shaders.get("triangle_vert");
        let frag = shaders.get("triangle_frag");

        let vert_stage = vk::PipelineShaderStageCreateInfo {
            stage: vk::ShaderStageFlags::VERTEX_BIT,
            module: vert.inner,
            p_name: vert.entry().as_ptr(),
            ..Default::default()
        };
        let frag_stage = vk::PipelineShaderStageCreateInfo {
            stage: vk::ShaderStageFlags::FRAGMENT_BIT,
            module: frag.inner,
            p_name: frag.entry().as_ptr(),
            ..Default::default()
        };
        let stages = vec![vert_stage, frag_stage];

        let layout_id = "null";
        let layout = pipe_layouts.get(layout_id).inner;

        let render_pass_id = "forward";
        let render_pass = render_passes.get(render_pass_id);
        let subpass_id = "color";
        let subpass = render_pass.subpasses[subpass_id];
        let render_pass = render_pass.inner;

        let vertex_input_state = Default::default();
        let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo {
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            ..Default::default()
        };

        let viewports = [swapchain.viewport()];
        let scissors = [swapchain.rect()];
        let viewport_state = vk::PipelineViewportStateCreateInfo {
            viewport_count: viewports.len() as _,
            p_viewports: viewports.as_ptr(),
            scissor_count: scissors.len() as _,
            p_scissors: scissors.as_ptr(),
            ..Default::default()
        };

        let rasterization_state = vk::PipelineRasterizationStateCreateInfo {
            cull_mode: vk::CullModeFlags::BACK_BIT,
            line_width: 1.0,
            ..Default::default()
        };

        let multisample_state = vk::PipelineMultisampleStateCreateInfo {
            rasterization_samples: vk::SampleCountFlags::_1_BIT,
            ..Default::default()
        };

        let color_blend_atts = [vk::PipelineColorBlendAttachmentState {
            color_write_mask: vk::ColorComponentFlags::R_BIT
                | vk::ColorComponentFlags::G_BIT
                | vk::ColorComponentFlags::B_BIT
                | vk::ColorComponentFlags::A_BIT,
            ..Default::default()
        }];
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo {
            attachment_count: color_blend_atts.len() as _,
            p_attachments: color_blend_atts.as_ptr(),
            ..Default::default()
        };

        let create_info = vk::GraphicsPipelineCreateInfo {
            stage_count: stages.len() as _,
            p_stages: stages.as_ptr(),
            p_vertex_input_state: &vertex_input_state,
            p_input_assembly_state: &input_assembly_state,
            p_viewport_state: &viewport_state,
            p_rasterization_state: &rasterization_state,
            p_multisample_state: &multisample_state,
            p_color_blend_state: &color_blend_state,
            layout,
            render_pass,
            subpass,
            ..Default::default()
        };
        let create_infos = std::slice::from_ref(&create_info);

        let mut inner = vk::null();
        let pipelines = std::slice::from_mut(&mut inner);
        dt.create_graphics_pipelines(
            vk::null(),                 // pipelineCache
            create_infos.len() as _,    // createInfoCount
            create_infos.as_ptr(),      // pCreateInfos
            ptr::null(),                // pAllocator
            pipelines.as_mut_ptr(),     // pPipelines
        ).check().unwrap();

        GraphicsPipeline {
            inner,
            layout: layout_id.to_owned(),
            render_pass: render_pass_id.to_owned(),
            subpass: subpass_id.to_owned(),
        }
    }
}

#[derive(Debug)]
struct AppState {
    dt: Arc<vkl::DeviceTable>,
    base: AppBase,
    res: Arc<AppResources>,
    pipelines: GraphicsPipelineManager<PipelineFactory>,
    cmd_pool: vk::CommandPool,
    cmds: vk::CommandBuffer,
}

impl Drop for AppState {
    fn drop(&mut self) {
        let dt = &*self.dt;
        unsafe {
            dt.device_wait_idle();
            dt.destroy_command_pool(self.cmd_pool, ptr::null());
        }
    }
}

unsafe fn init_state(res: Arc<AppResources>) -> AppState {
    let gfx_queue = Arc::clone(&res.queues[0][0]);
    let device = Arc::clone(&gfx_queue.device);
    let dt = Arc::clone(&device.table);

    let base = AppBase::new(Arc::clone(&res));

    let factory = PipelineFactory::new(Arc::clone(&res));
    let pipelines = GraphicsPipelineManager::new(Arc::clone(&device), factory);

    let create_info = vk::CommandPoolCreateInfo {
        flags: vk::CommandPoolCreateFlags::TRANSIENT_BIT,
        queue_family_index: gfx_queue.family.index,
        ..Default::default()
    };
    let mut cmd_pool = vk::null();
    dt.create_command_pool(&create_info, ptr::null(), &mut cmd_pool);

    AppState {
        dt,
        base,
        res,
        pipelines,
        cmd_pool,
        cmds: vk::null(),
    }
}

impl AppState {
    unsafe fn record_cmds(&mut self) {
        let dt = &*self.dt;

        let cmd_pool = self.cmd_pool;
        dt.reset_command_pool(cmd_pool, Default::default());

        let alloc_info = vk::CommandBufferAllocateInfo {
            command_pool: cmd_pool,
            command_buffer_count: 1,
            ..Default::default()
        };
        dt.allocate_command_buffers(&alloc_info, &mut self.cmds);

        let cmds = self.cmds;
        let begin_info = vk::CommandBufferBeginInfo {
            flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT_BIT,
            ..Default::default()
        };
        dt.begin_command_buffer(cmds, &begin_info);

        let render_pass = self.res.render_passes.get("forward").inner;
        let framebuffer = self.base.cur_framebuffer();
        let render_area = self.res.framebuffers.rect();
        let begin_info = vk::RenderPassBeginInfo {
            render_pass,
            framebuffer,
            render_area,
            ..Default::default()
        };
        let contents = vk::SubpassContents::INLINE;
        dt.cmd_begin_render_pass(cmds, &begin_info, contents);

        let pipeline = self.pipelines.get(&()).inner;
        dt.cmd_bind_pipeline(cmds, vk::PipelineBindPoint::GRAPHICS, pipeline);
        dt.cmd_draw(cmds, 3, 1, 0, 0);

        dt.cmd_end_render_pass(cmds);

        dt.end_command_buffer(cmds);
    }

    unsafe fn submit_cmds(&mut self) {
        let cmd_bufs = [self.cmds];
        let wait_sems = [self.base.acquire_sem];
        let wait_masks = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT_BIT];
        let sig_sems = [self.base.render_sem];
        let submit_infos = [vk::SubmitInfo {
            command_buffer_count: cmd_bufs.len() as _,
            p_command_buffers: cmd_bufs.as_ptr(),
            wait_semaphore_count: wait_sems.len() as _,
            p_wait_semaphores: wait_sems.as_ptr(),
            p_wait_dst_stage_mask: wait_masks.as_ptr(),
            signal_semaphore_count: sig_sems.len() as _,
            p_signal_semaphores: sig_sems.as_ptr(),
            ..Default::default()
        }];
        self.base.gfx_queue.submit(&submit_infos[..], self.base.render_fence);
    }
}

unsafe fn render_main(ev_proxy: window::EventLoopProxy) {
    let (swapchain, queues) = init_video(&ev_proxy, "triangle demo");
    let res = Arc::new(init_resources(swapchain, queues));
    let mut state = init_state(res);

    while !state.res.window.should_close() {
        state.base.acquire_next_image();
        state.base.wait_for_render();
        state.record_cmds();
        state.submit_cmds();
        state.base.present();
    }
}

fn main() {
    unsafe { with_event_loop(|proxy| render_main(proxy)); }
}
