use std::sync::Arc;

use crate::*;

/// Top-level renderer.
#[derive(Debug)]
crate struct WorldRenderer {
    globals: Arc<Globals>,
    scheduler: Scheduler,
    pass: Arc<TrivialPass>,
    framebuffers: Vec<Arc<Framebuffer>>,
    debug: Option<Box<DebugRenderer>>,
}

impl WorldRenderer {
    crate fn new(
        state: &SystemState,
        globals: Arc<Globals>,
        swapchain: &Swapchain,
        scheduler: Scheduler,
    ) -> Self {
        let pass = Arc::new(TrivialPass::new(Arc::clone(&state.device)));
        let framebuffers = pass.create_framebuffers(&swapchain);
        let debug = DebugRenderer::new(state, Arc::clone(&globals));
        Self {
            globals,
            scheduler,
            pass,
            framebuffers,
            debug: Some(Box::new(debug)),
        }
    }

    crate fn invalidate_swapchain(&mut self, _new_swapchain: &Swapchain) {
        todo!();
    }

    crate fn run(
        &mut self,
        state: Arc<SystemState>,
        world: RenderWorld,
        _frame_num: u64,
        swapchain_image: u32,
        present_sem: &mut Semaphore,
        render_fence: &mut Fence,
        render_sem: &mut Semaphore,
    ) {
        unsafe { self.scheduler.clear(); }

        let view = SceneView::new(state, &world);

        let framebuffer =
            Arc::clone(&self.framebuffers[swapchain_image as usize]);
        let mut pass = RenderPassNode::new(framebuffer);

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
