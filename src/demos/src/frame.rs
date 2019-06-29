use std::ptr;
use std::sync::Arc;

use crate::*;

#[derive(Debug)]
pub struct FrameState {
    pub dt: Arc<vkl::DeviceTable>,
    pub path: Arc<RenderPath>,
    pub cmd_pool: vk::CommandPool,
    pub cmds: vk::CommandBuffer,
    pub timer: FrameTimer,
    pub done_sem: vk::Semaphore,
    pub done_fence: vk::Fence,
    pub sprite_buf: SpriteBuffer,
    pub sprite_set: vk::DescriptorSet,
    pub framebuf_idx: u32,
    pub sprite_count: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FrameLog {
    pub time_ns: f32,
}

const SPRITE_BUF_SIZE: u32 = 2048;

unsafe fn prepare_descriptor_sets(
    path: &RenderPath,
    res: &mut InitResources,
    sprite_buffers: &[SpriteBuffer; 2],
) -> [vk::DescriptorSet; 2] {
    let objs = &mut res.objs;

    let set_layout = &path.sprite_set_layout;
    let params = CreateDescriptorSetParams {
        count: 2,
        ..Default::default()
    };
    let (_, mut sets) = create_descriptor_sets(objs, set_layout, params);

    for (&set, sbuf) in sets.iter().zip(sprite_buffers.iter()) {
        let buf_writes = [vk::DescriptorBufferInfo {
            buffer: sbuf.buffer,
            offset: sbuf.offset,
            range: sbuf.range,
        }];
        let writes = [vk::WriteDescriptorSet {
            dst_set: set,
            dst_binding: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
            p_buffer_info: buf_writes.as_ptr(),
            ..Default::default()
        }];
        objs.device.table.update_descriptor_sets(
            writes.len() as _,
            writes.as_ptr(),
            0,
            ptr::null(),
        );
    }

    [sets.pop().unwrap(), sets.pop().unwrap()]
}

impl FrameState {
    pub unsafe fn new_pair(path: Arc<RenderPath>, res: &mut InitResources) ->
        [Self; 2]
    {
        let device = &path.swapchain.device;

        let sprite_bufs = SpriteBuffer::new_pair(res, SPRITE_BUF_SIZE);
        let desc_sets = prepare_descriptor_sets(&path, res, &sprite_bufs);

        let objs = &mut res.objs;
        let mut create_frame = |sprite_buf, sprite_set| {
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

            let timer = FrameTimer::new(objs);

            let done_sem = objs.create_semaphore();
            let done_fence = objs.create_fence(true);

            FrameState {
                dt: Arc::clone(&device.table),
                path: Arc::clone(&path),
                cmd_pool,
                cmds,
                timer,
                done_sem,
                done_fence,
                sprite_buf,
                sprite_set,
                framebuf_idx: 0,
                sprite_count: 0,
            }
        };

        let [s0, s1] = sprite_bufs;
        let [d0, d1] = desc_sets;
        [create_frame(s0, d0), create_frame(s1, d1)]
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
            self.path.sprite_pipeline,
        );

        let descriptors = std::slice::from_ref(&self.sprite_set);
        dt.cmd_bind_descriptor_sets(
            cb,                                 // commandBuffer
            vk::PipelineBindPoint::GRAPHICS,    // pipelineBindPoint
            self.path.sprite_pipeline_layout,   // layout
            0,                                  // firstSet
            descriptors.len() as _,             // descriptorSetCount
            descriptors.as_ptr(),               // pDescriptorSets
            0,                                  // dynamicOffsetCount
            ptr::null(),                        // pDynamicOffsets
        );
        dt.cmd_draw(cb, 4, self.sprite_count, 0, 0);

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
