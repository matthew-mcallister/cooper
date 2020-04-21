use std::sync::Arc;

use crate::*;

/// Top-level renderer.
#[derive(Debug)]
crate struct WorldRenderer {
    globals: Arc<Globals>,
    scheduler: Scheduler,
    pass: Arc<TrivialPass>,
    framebuffers: Vec<Arc<Framebuffer>>,
    clear_values: [vk::ClearValue; 1],
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
        let clear_values = [vk::ClearValue {
            color: vk::ClearColorValue { float_32: [0.0; 4], },
        }];
        let debug = DebugRenderer::new(state, Arc::clone(&globals));
        Self {
            globals,
            scheduler,
            pass,
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
