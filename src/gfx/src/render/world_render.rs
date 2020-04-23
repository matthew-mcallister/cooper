use std::sync::Arc;

use crate::*;

#[derive(Debug)]
crate struct BasicPass {
    crate pass: Arc<RenderPass>,
    crate subpass: Subpass,
}

impl BasicPass {
    crate fn new(device: Arc<Device>) -> Self {
        let pass = unsafe { create_basic_pass(device) };
        let mut subpasses = pass.subpasses();
        BasicPass {
            pass: Arc::clone(&pass),
            subpass: subpasses.next().unwrap(),
        }
    }

    crate fn create_framebuffers(
        &self,
        state: &SystemState,
        swapchain: &Swapchain,
    ) -> Vec<Arc<Framebuffer>> {
        unsafe {
            swapchain.create_views().into_iter()
                .map(|view| {
                    let depth_view = create_render_target(
                        state,
                        &self.pass,
                        1,
                        swapchain.extent(),
                        false,
                    );
                    Arc::new(Framebuffer::new(
                        Arc::clone(&self.pass),
                        vec![view.into(), depth_view.into()],
                    ))
                })
                .collect()
        }
    }
}

unsafe fn create_basic_pass(device: Arc<Device>) -> Arc<RenderPass> {
    use vk::ImageLayout as Il;
    RenderPass::new(
        device,
        vec![
            AttachmentDescription {
                name: Attachment::Backbuffer,
                format: Format::BGRA8_SRGB,
                load_op: vk::AttachmentLoadOp::CLEAR,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                ..Default::default()
            },
            AttachmentDescription {
                name: Attachment::DepthStencil,
                format: Format::D32F,
                load_op: vk::AttachmentLoadOp::CLEAR,
                // TODO: Maybe initial_layout should equal final_layout.
                // But that would require a manual layout transition
                // before the first frame.
                final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                ..Default::default()
            },
        ],
        vec![
            SubpassDesc {
                layouts: vec![
                    Il::COLOR_ATTACHMENT_OPTIMAL,
                    Il::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                ],
                color_attchs: vec![0],
                depth_stencil_attch: Some(1),
                ..Default::default()
            },
        ],
        vec![],
    )
}

/// Top-level renderer.
#[derive(Debug)]
crate struct WorldRenderer {
    globals: Arc<Globals>,
    scheduler: Scheduler,
    basic_pass: BasicPass,
    framebuffers: Vec<Arc<Framebuffer>>,
    clear_values: [vk::ClearValue; 2],
    debug: Option<Box<DebugRenderer>>,
}

impl WorldRenderer {
    crate fn new(
        state: &SystemState,
        globals: Arc<Globals>,
        swapchain: &Swapchain,
        scheduler: Scheduler,
    ) -> Self {
        let basic_pass = BasicPass::new(Arc::clone(&state.device));
        let framebuffers = basic_pass.create_framebuffers(state, &swapchain);
        let clear_values = [clear_color([0.0; 4]), clear_depth(0.0)];
        let debug = DebugRenderer::new(state, Arc::clone(&globals));
        Self {
            globals,
            scheduler,
            basic_pass,
            framebuffers,
            clear_values,
            debug: Some(Box::new(debug)),
        }
    }

    /// Used when recreating the swapchain
    crate fn into_inner(self) -> Scheduler {
        self.scheduler
    }

    crate fn run(
        &mut self,
        state: Arc<Box<SystemState>>,
        world: RenderWorld,
        _frame_num: u64,
        swapchain_image: u32,
        present_sem: &mut Semaphore,
        render_fence: &mut Fence,
        render_sem: &mut Semaphore,
    ) {
        unsafe { self.scheduler.clear(); }

        let framebuffer =
            Arc::clone(&self.framebuffers[swapchain_image as usize]);
        let clear_values = self.clear_values.to_vec();
        let mut pass = RenderPassNode::with_clear(framebuffer, clear_values);

        let view = SceneViewState::new(state, &world);
        let mut debug = self.debug.take().unwrap();
        let (debug_return, task) = subpass_task(move |cmds| {
            debug.render(&view, world.debug, cmds);
            debug
        });
        pass.add_task(0, task);

        self.scheduler.schedule_pass(
            pass,
            &[present_sem],
            &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT_BIT],
            &[render_sem],
            Some(render_fence),
        );

        self.debug = Some(debug_return.take().unwrap());
    }
}
