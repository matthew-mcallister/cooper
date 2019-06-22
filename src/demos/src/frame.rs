use std::sync::Arc;

use crate::*;

#[derive(Debug)]
pub struct FrameState {
    pub dt: Arc<vkl::DeviceTable>,
    pub path: Arc<RenderPath>,
    pub objs: Box<ObjectTracker>,
    pub cmd_pool: vk::CommandPool,
    pub timer: FrameTimer,
    pub done_sem: vk::Semaphore,
    pub done_fence: vk::Fence,
    pub framebuf_idx: u32,
    pub cmds: vk::CommandBuffer,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FrameLog {
    pub time_ns: f32,
}

impl FrameState {
    pub unsafe fn new(path: Arc<RenderPath>) -> Self {
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

    pub unsafe fn record(&mut self) {
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

    pub unsafe fn submit(&mut self, queue: vk::Queue, wait_sem: vk::Semaphore)
    {
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

    pub unsafe fn wait_until_done(&mut self) {
        self.dt.wait_for_fences
            (1, &self.done_fence as _, vk::FALSE, u64::max_value())
            .check_success().unwrap();
        self.dt.reset_fences(1, &self.done_fence as _).check().unwrap();
    }

    pub unsafe fn collect_log(&mut self) -> FrameLog {
        // Gather statistics after rendering
        let ts = self.timer.get_query_results();
        let time_ns = ts.to_ns(&self.path.swapchain.device);
        FrameLog { time_ns }
    }
}
