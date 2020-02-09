use std::sync::Arc;

use crate::*;

/// Top-level renderer.
#[derive(Debug)]
crate struct WorldRenderer {
    globals: Arc<Globals>,
    scheduler: Scheduler,
    pass: Arc<TrivialPass>,
    framebuffers: Vec<Arc<Framebuffer>>,
    trivial: Option<Box<TrivialRenderer>>,
}

impl WorldRenderer {
    crate fn new(
        state: &SystemState,
        globals: Arc<Globals>,
        swapchain: &Swapchain,
        scheduler: Scheduler,
    ) -> Self {
        let pass = Arc::new(TrivialPass::new(Arc::clone(&state.device)));
        let trivial =
            Box::new(TrivialRenderer::new(state, Arc::clone(&globals)));
        let framebuffers = pass.create_framebuffers(&swapchain);
        Self {
            globals,
            scheduler,
            pass,
            framebuffers,
            trivial: Some(trivial),
        }
    }

    crate fn invalidate_swapchain(&mut self, _new_swapchain: &Swapchain) {
        todo!();
    }

    crate fn run(
        &mut self,
        state: Arc<SystemState>,
        _frame_num: u64,
        swapchain_image: u32,
        present_sem: &mut Semaphore,
        render_fence: &mut Fence,
        render_sem: &mut Semaphore,
    ) {
        let framebuffer =
            Arc::clone(&self.framebuffers[swapchain_image as usize]);
        let mut pass = RenderPassNode::new(framebuffer);

        let mut trivial = self.trivial.take().unwrap();
        let (trivial_return, task) = subpass_task(move |cmds| {
            trivial.render(&state, cmds);
            trivial
        });
        pass.add_task(0, task);

        self.scheduler.schedule_pass(
            pass,
            &[present_sem],
            &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT_BIT],
            &[render_sem],
            Some(render_fence),
        );

        self.trivial = Some(trivial_return.take().unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let device = Arc::clone(vars.device());
        let gfx_queue = Arc::clone(&vars.gfx_queue);
        let mut swapchain = vars.swapchain;
        let state = Arc::new(SystemState::new(Arc::clone(&device)));
        let globals = Arc::new(Globals::new(&state));
        let scheduler = Scheduler::new(Arc::clone(&gfx_queue));

        let mut renderer = WorldRenderer::new(
            &state,
            globals,
            &swapchain,
            scheduler,
        );

        let mut swapchain_sem = Semaphore::new(Arc::clone(&device));
        let image_idx = swapchain.acquire_next_image(&mut swapchain_sem)
            .unwrap();

        let mut render_fence = Fence::new(Arc::clone(&device), false);
        let mut render_sem = Semaphore::new(Arc::clone(&device));
        renderer.run(
            Arc::clone(&state),
            1,
            image_idx,
            &mut swapchain_sem,
            &mut render_fence,
            &mut render_sem,
        );

        gfx_queue.present(
            &[&render_sem],
            &mut swapchain,
            image_idx,
        ).check().unwrap();
        render_fence.wait();
        render_fence.reset();
    }

    unit::declare_tests![smoke_test];
}

unit::collect_tests![tests];
