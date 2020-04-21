use std::sync::Arc;

use crossbeam::atomic::AtomicCell;
use derivative::Derivative;

use crate::*;

crate type SubpassTask = Box<dyn FnOnce(&mut SubpassCmds) + Send>;

#[derive(Debug)]
crate struct Scheduler {
    pool: Option<Box<CmdPool>>,
    // List of buffers to free each frame.
    buffers: Vec<vk::CommandBuffer>,
    gfx_queue: Arc<Queue>,
}

#[derive(Derivative)]
#[derivative(Debug)]
crate struct RenderPassNode {
    framebuffer: Arc<Framebuffer>,
    clear_values: Vec<vk::ClearValue>,
    // An array of tasks per subpass
    // TODO: Maybe tasks should be named for debug output purposes
    // All rendering nodes really
    #[derivative(Debug = "ignore")]
    tasks: Vec<Vec<SubpassTask>>,
}

impl RenderPassNode {
    crate fn new(framebuffer: Arc<Framebuffer>) -> Self {
        Self::with_clear(framebuffer, Vec::new())
    }

    crate fn with_clear(
        framebuffer: Arc<Framebuffer>,
        clear_values: Vec<vk::ClearValue>,
    ) -> Self {
        let tasks: Vec<_> = (0..framebuffer.pass().subpasses().len())
            .map(|_| Vec::new())
            .collect();

        // validate
        for (i, _) in framebuffer.pass().attachments().iter().enumerate()
            .filter(|(_, attch)| attch.load_op == vk::AttachmentLoadOp::CLEAR)
        {
            assert!(clear_values.len() > i);
        }

        RenderPassNode {
            framebuffer,
            clear_values,
            tasks,
        }
    }

    crate fn add_task(&mut self, subpass: usize, task: SubpassTask) {
        self.tasks[subpass].push(task.into())
    }
}

impl Scheduler {
    crate fn new(gfx_queue: Arc<Queue>) -> Self {
        assert!(gfx_queue.family().supports_graphics());
        let flags = vk::CommandPoolCreateFlags::TRANSIENT_BIT;
        let pool = Box::new(CmdPool::new(gfx_queue.family(), flags));
        Self {
            pool: Some(pool),
            buffers: Vec::new(),
            gfx_queue,
        }
    }

    crate fn schedule_pass(
        &mut self,
        pass: RenderPassNode,
        wait_sems: &[&Semaphore],
        wait_stages: &[vk::PipelineStageFlags],
        sig_sems: &[&Semaphore],
        fence: Option<&mut Fence>,
    ) {
        let pool = self.pool.take().unwrap();
        let cmds = RenderPassCmds::new(
            CmdBuffer::new(pool, CmdBufferLevel::Primary),
            pass.framebuffer,
            &pass.clear_values,
            SubpassContents::Inline,
        );

        let mut pass_cmds = Some(cmds);
        for (i, subpass) in pass.tasks.into_iter().enumerate() {
            let cmds = pass_cmds.take().unwrap().enter_subpass();
            let mut cmds = if i > 0 {
                let mut cmds = cmds.exit_subpass();
                cmds.next_subpass(SubpassContents::Inline);
                cmds.enter_subpass()
            } else { cmds };

            for task in subpass.into_iter() {
                task(&mut cmds);
            }

            pass_cmds = Some(cmds.exit_subpass());
        }

        let (cmds, pool) = pass_cmds.unwrap().end().end();
        self.buffers.push(cmds);
        unsafe {
            self.gfx_queue.submit(&[SubmitInfo {
                wait_sems,
                wait_stages,
                sig_sems,
                cmds: &[cmds],
            }], fence);
        }

        self.pool = Some(pool);
    }

    crate unsafe fn clear(&mut self) {
        let pool = self.pool.as_mut().unwrap();
        pool.reset();
        pool.free(&self.buffers);
        self.buffers.clear();
    }
}

crate type SharedBox<T> = Arc<AtomicCell<Option<Box<T>>>>;

/// Creates a task with an asynchronous return value.
crate fn subpass_task<T, V, F>(f: F) -> (SharedBox<T>, SubpassTask)
where
    T: Send + 'static,
    V: Into<Box<T>>,
    F: FnOnce(&mut SubpassCmds) -> V + Send + 'static,
{
    let ret = Arc::new(AtomicCell::new(None));
    let ret2 = Arc::clone(&ret);
    let g: SubpassTask = Box::new(move |x| { ret2.store(Some(f(x).into())); });
    (ret, g)
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let device = Arc::clone(vars.device());
        let state = Arc::new(SystemState::new(Arc::clone(&device)));
        let globals = Arc::new(Globals::new(&state));
        let pass = TrivialPass::new(Arc::clone(&device));
        let mut trivial = TrivialRenderer::new(&state, Arc::clone(&globals));

        let framebuffers = pass.create_framebuffers(&vars.swapchain);

        let mut pass = RenderPassNode::new(Arc::clone(&framebuffers[0]));
        let state_ = Arc::clone(&state);
        pass.add_task(0, Box::new(move |cmds| trivial.render(&state_, cmds)));

        let mut scheduler = Scheduler::new(Arc::clone(&vars.gfx_queue));
        scheduler.schedule_pass(pass, &[], &[], &[], None);

        device.wait_idle();
    }

    unit::declare_tests![smoke_test];
}

unit::collect_tests![tests];
