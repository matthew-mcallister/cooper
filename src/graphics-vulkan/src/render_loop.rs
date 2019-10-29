use std::os::raw::c_int;
use std::ptr;
use std::sync::Arc;

use ccore::name::*;
use prelude::*;

use crate::*;

#[derive(Debug)]
pub struct RenderLoop {
    window: Arc<window::Window>,
    device: Arc<Device>,
    swapchain: Swapchain,
    gfx_queue: Arc<Queue>,
    core: Arc<CoreData>,
    framebuffers: Vec<Arc<Framebuffer>>,
    frame_num: u64,
    swap_img_idx: u32,
    cmd_pool: vk::CommandPool,
    // Stuff that should be frame local
    cmds: vk::CommandBuffer,
    acquire_sem: vk::Semaphore,
    render_fence: vk::Fence,
    render_sem: vk::Semaphore,
}

impl Drop for RenderLoop {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            dt.device_wait_idle();
            dt.destroy_command_pool(self.cmd_pool, ptr::null());
            dt.destroy_semaphore(self.acquire_sem, ptr::null());
            dt.destroy_fence(self.render_fence, ptr::null());
            dt.destroy_semaphore(self.render_sem, ptr::null());
        }
    }
}

impl RenderLoop {
    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    pub unsafe fn new(
        ev: &window::EventLoopProxy,
        app_info: AppInfo,
        config: Config,
    ) -> Result<Self, AnyError> {
        // Init window
        let create_info = window::CreateInfo {
            title: app_info.name.clone(),
            dims: (config.width as c_int, config.height as c_int).into(),
            hints: Default::default(),
        };
        let window = Arc::new(ev.create_window(create_info)?);

        // Init device
        let (swapchain, queues) =
            init_swapchain(app_info, Arc::clone(&window))?;
        let device = Arc::clone(swapchain.device());

        let mut core = CoreData::new(Arc::clone(&device), &queues, config);
        core.init();
        let core = Arc::new(core);

        let gfx_queue = Arc::clone(&queues[0][0]);

        let pass = Name::new("forward");
        let attachments = Attachment::from_swapchain(&swapchain).map(Arc::new);
        let framebuffers: Vec<_> = attachments.map(|attachment| {
            Arc::new(Framebuffer::new(&core, pass, vec![attachment]))
        }).collect();

        let dt = &*device.table;

        let create_info = vk::CommandPoolCreateInfo {
            flags: vk::CommandPoolCreateFlags::TRANSIENT_BIT
                | vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER_BIT,
            queue_family_index: gfx_queue.family().index(),
            ..Default::default()
        };
        let mut cmd_pool = vk::null();
        dt.create_command_pool(&create_info, ptr::null(), &mut cmd_pool);

        let alloc_info = vk::CommandBufferAllocateInfo {
            command_pool: cmd_pool,
            level: vk::CommandBufferLevel::PRIMARY,
            command_buffer_count: 1,
            ..Default::default()
        };
        let mut cmds = vk::null();
        dt.allocate_command_buffers(&alloc_info, &mut cmds);

        let render_fence = device.create_fence(true);
        let render_sem = device.create_semaphore();
        let acquire_sem = device.create_semaphore();

        Ok(RenderLoop {
            window,
            device,
            swapchain,
            gfx_queue,
            core,
            framebuffers,
            frame_num: 0,
            swap_img_idx: 0,
            cmd_pool,
            cmds,
            acquire_sem,
            render_fence,
            render_sem,
        })
    }

    pub fn window(&self) -> &Arc<window::Window> {
        &self.window
    }

    fn framebuffer(&self) -> &Arc<Framebuffer> {
        &self.framebuffers[self.swap_img_idx as usize]
    }

    fn frame_info(&self) -> FrameInfo {
        FrameInfo::new(
            Arc::clone(&self.core),
            Arc::clone(self.framebuffer()),
            self.frame_num,
        )
    }

    unsafe fn record_commands(&mut self) {
        let frame_info = Arc::new(self.frame_info());

        let triangle_cmds = triangle_task(Arc::clone(&frame_info)).cmds;

        let dt = &*self.device().table;
        let cmds = self.cmds;

        let begin_info = vk::CommandBufferBeginInfo {
            flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT_BIT,
            ..Default::default()
        };
        dt.begin_command_buffer(cmds, &begin_info);

        let pass = self.core.get_pass(Name::new("forward"));

        let framebuffer = frame_info.framebuffer();
        let clear_values = [clear_color([0.0; 4])];
        let begin_info = vk::RenderPassBeginInfo {
            render_pass: pass.inner(),
            framebuffer: framebuffer.inner(),
            render_area: framebuffer.render_area(),
            clear_value_count: clear_values.len() as _,
            p_clear_values: clear_values.as_ptr(),
            ..Default::default()
        };
        let contents = vk::SubpassContents::SECONDARY_COMMAND_BUFFERS;
        dt.cmd_begin_render_pass(cmds, &begin_info, contents);

        let subcmds = [triangle_cmds.inner()];
        dt.cmd_execute_commands(cmds, subcmds.len() as _, subcmds.as_ptr());

        dt.cmd_end_render_pass(cmds);

        dt.end_command_buffer(cmds);
    }

    unsafe fn acquire_image(&mut self) {
        let dt = &*self.device.table;
        dt.acquire_next_image_khr(
            self.swapchain.inner(), //swapchain
            u64::max_value(),       //timeout
            self.acquire_sem,       //semaphore
            vk::null(),             //fence
            &mut self.swap_img_idx, //pImageIndex
        );
    }

    // TODO: Probably make async
    unsafe fn wait_for_render(&self) {
        let dt = &*self.device().table;
        let fences = [self.render_fence];
        dt.wait_for_fences(
            fences.len() as _,
            fences.as_ptr(),
            vk::TRUE,
            u64::max_value(),
        ).check().unwrap();
        dt.reset_fences(fences.len() as _, fences.as_ptr());
    }

    unsafe fn submit(&self) {
        let wait_sems = [self.acquire_sem];
        let wait_masks = [vk::PipelineStageFlags::FRAGMENT_SHADER_BIT];
        let cmd_bufs = [self.cmds];
        let sig_sems = [self.render_sem];
        let submissions = [vk::SubmitInfo {
            wait_semaphore_count: wait_sems.len() as _,
            p_wait_semaphores: wait_sems.as_ptr(),
            p_wait_dst_stage_mask: wait_masks.as_ptr(),
            command_buffer_count: cmd_bufs.len() as _,
            p_command_buffers: cmd_bufs.as_ptr(),
            signal_semaphore_count: sig_sems.len() as _,
            p_signal_semaphores: sig_sems.as_ptr(),
            ..Default::default()
        }];
        self.gfx_queue.submit(&submissions, self.render_fence);
    }

    unsafe fn present(&self) {
        let wait_sems = [self.render_sem];
        let swapchains = [self.swapchain.inner()];
        let image_indices = [self.swap_img_idx];
		let present_info = vk::PresentInfoKHR {
            wait_semaphore_count: wait_sems.len() as _,
            p_wait_semaphores: wait_sems.as_ptr(),
            swapchain_count: swapchains.len() as _,
            p_swapchains: swapchains.as_ptr(),
            p_image_indices: image_indices.as_ptr(),
            ..Default::default()
        };
        self.gfx_queue.present(&present_info);
    }

    pub unsafe fn do_frame(&mut self) {
        self.frame_num += 1;
        self.acquire_image();
        self.wait_for_render();
        self.record_commands();
        self.submit();
        self.present();
    }
}
