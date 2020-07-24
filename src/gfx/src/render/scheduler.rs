use std::sync::Arc;

use crossbeam::atomic::AtomicCell;
use derivative::Derivative;

use crate::*;

crate type SubpassTask = Box<dyn FnOnce(&mut SubpassCmds) + Send>;

#[derive(Debug)]
crate struct RenderScheduler {
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
        // Can't use the vec! macro here...
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
        self.tasks[subpass].push(task)
    }
}

impl RenderScheduler {
    crate fn new(gfx_queue: Arc<Queue>) -> Self {
        assert!(gfx_queue.family().supports_graphics());
        let flags = vk::CommandPoolCreateFlags::TRANSIENT_BIT;
        let mut pool = Box::new(CmdPool::new(gfx_queue.family(), flags));
        pool.set_name("render_scheduler.pool");
        Self {
            pool: Some(pool),
            buffers: Vec::new(),
            gfx_queue,
        }
    }

    crate fn schedule_pass(
        &mut self,
        pass: RenderPassNode,
        // TODO: Ought to abstract over these arguments
        wait_sems: &[WaitInfo<'_>],
        sig_sems: &[SignalInfo<'_>],
    ) {
        let pool = self.pool.take().unwrap();
        let mut pass_cmds = RenderPassCmds::new(
            CmdBuffer::new(pool, CmdBufferLevel::Primary),
            pass.framebuffer,
            &pass.clear_values,
            SubpassContents::Inline,
        );

        for (i, subpass) in pass.tasks.into_iter().enumerate() {
            if i > 0 {
                pass_cmds.next_subpass(SubpassContents::Inline);
            }

            let mut cmds = pass_cmds.enter_subpass();
            for task in subpass.into_iter() {
                task(&mut cmds);
            }
            pass_cmds = cmds.exit_subpass();
        }

        let (cmds, pool) = pass_cmds.end().end();
        self.buffers.push(cmds);
        unsafe {
            self.gfx_queue.submit(&[SubmitInfo {
                wait_sems,
                sig_sems,
                cmds: &[cmds],
            }]);
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
        let heap = ImageHeap::new(Arc::clone(&device));
        let globals = Arc::new(Globals::new(&state, &heap));
        let pass = TrivialPass::new(Arc::clone(&device));
        let trivial = TrivialRenderer::new(&state, Arc::clone(&globals));

        let framebuffers = pass.create_framebuffers(&vars.swapchain);

        let mut pass = RenderPassNode::new(Arc::clone(&framebuffers[0]));
        let state_ = Arc::clone(&state);
        pass.add_task(0, Box::new(move |cmds| trivial.render(&state_, cmds)));

        let mut scheduler = RenderScheduler::new(Arc::clone(&vars.gfx_queue));
        scheduler.schedule_pass(pass, &[], &[]);

        device.wait_idle();
    }

    unit::declare_tests![smoke_test];
}

unit::collect_tests![tests];
