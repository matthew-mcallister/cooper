use std::sync::Arc;

use ccore::name::*;

use crate::*;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CmdBufferState {
    Initial,
    Recording,
    Executable,
    Pending,
}

impl Default for CmdBufferState {
    fn default() -> Self {
        CmdBufferState::Initial
    }
}

// TODO: Ensure command pool thread safety while recording. Options
// - Hold a mut reference to the command pool
// - Acquire a lock (without waiting)
// - Make the command pool and command buffer !Send
// TODO: Ensure command buffer is not used after command pool is reset.
// Should be solved by a monotonic counter.
// TODO: Merge all the specialized versions into one?
#[derive(Debug)]
crate struct CmdBuffer {
    device: Arc<Device>,
    state: CmdBufferState,
    pool: vk::CommandPool,
    inner: vk::CommandBuffer,
}

impl CmdBuffer {
    fn dt(&self) -> &vkl::DeviceTable {
        &self.device.table
    }

    fn inner(&self) -> vk::CommandBuffer {
        self.inner
    }

    fn ensure_recording(&self) {
        assert_eq!(self.state, CmdBufferState::Recording);
    }

    unsafe fn new(
        device: Arc<Device>,
        command_pool: vk::CommandPool,
        level: vk::CommandBufferLevel,
    ) -> Self {
        let dt = &*device.table;
        let alloc_info = vk::CommandBufferAllocateInfo {
            command_pool,
            level,
            command_buffer_count: 1,
            ..Default::default()
        };
        let mut inner = vk::null();
        dt.allocate_command_buffers(&alloc_info, &mut inner);

        CmdBuffer {
            device,
            state: CmdBufferState::Initial,
            pool: command_pool,
            inner,
        }
    }

    unsafe fn begin(&mut self, begin_info: &vk::CommandBufferBeginInfo) {
        assert_eq!(self.state, CmdBufferState::Initial);
        self.dt().begin_command_buffer(self.inner, begin_info);
        self.state = CmdBufferState::Recording;
    }

    unsafe fn bind_graphics_pipeline(&mut self, pipeline: vk::Pipeline) {
        self.ensure_recording();
        self.dt().cmd_bind_pipeline(
            self.inner,
            vk::PipelineBindPoint::GRAPHICS,
            pipeline,
        );
    }

    unsafe fn draw(
        &mut self,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) {
        self.ensure_recording();
        self.dt().cmd_draw(
            self.inner,
            vertex_count,
            instance_count,
            first_vertex,
            first_instance,
        );
    }

    unsafe fn end(&mut self) {
        assert_eq!(self.state, CmdBufferState::Recording);
        self.dt().end_command_buffer(self.inner);
    }
}

#[derive(Debug)]
crate struct SubpassCmds {
    core: Arc<CoreData>,
    inner: CmdBuffer,
    framebuffer: Arc<Framebuffer>,
    pass: Name,
    subpass: Name,
    bound_pipeline: Option<PipelineDesc>,
}

#[derive(Debug)]
crate struct SubpassCmdsCreateInfo {
    crate core: Arc<CoreData>,
    crate pool: vk::CommandPool,
    crate framebuffer: Arc<Framebuffer>,
    crate pass: Name,
    crate subpass: Name,
}

impl SubpassCmds {
    crate fn inner(&self) -> vk::CommandBuffer {
        self.inner.inner()
    }

    crate unsafe fn new(create_info: SubpassCmdsCreateInfo) -> Self {
        let inner = CmdBuffer::new(
            Arc::clone(create_info.core.device()),
            create_info.pool,
            vk::CommandBufferLevel::SECONDARY,
        );
        SubpassCmds {
            core: create_info.core,
            inner,
            framebuffer: create_info.framebuffer,
            pass: create_info.pass,
            subpass: create_info.subpass,
            bound_pipeline: None,
        }
    }

    crate unsafe fn begin(&mut self) {
        let pass = self.core.get_pass(self.pass);
        let subpass = pass.get_subpass(self.subpass);
        let inheritance_info = vk::CommandBufferInheritanceInfo {
            render_pass: pass.inner(),
            subpass,
            framebuffer: self.framebuffer.inner(),
            ..Default::default()
        };
        let begin_info = vk::CommandBufferBeginInfo {
            flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT_BIT
                | vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE_BIT,
            p_inheritance_info: &inheritance_info,
            ..Default::default()
        };
        self.inner.begin(&begin_info);
    }

    crate unsafe fn bind_graphics_pipeline(&mut self, desc: &PipelineDesc) {
        if Some(desc) == self.bound_pipeline.as_ref() { return; }
        let pipe = self.core.get_pipeline(desc);
        assert_eq!(self.pass, pipe.pass());
        assert_eq!(self.subpass, pipe.subpass());
        self.inner.bind_graphics_pipeline(pipe.inner());
        self.bound_pipeline = Some(desc.clone());
    }

    crate unsafe fn draw(
        &mut self,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) {
        assert!(self.bound_pipeline.is_some());
        // TODO: Assert a mesh is bound if needed by the pipeline
        self.inner.draw(
            vertex_count,
            instance_count,
            first_vertex,
            first_instance,
        );
    }

    crate unsafe fn end(&mut self) {
        self.inner.end();
    }
}
