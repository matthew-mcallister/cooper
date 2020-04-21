use std::ptr;
use std::sync::Arc;

use derivative::Derivative;
use log::trace;
use prelude::*;

use crate::*;

#[derive(Clone, Copy, Debug, Derivative, Eq, Hash, PartialEq)]
#[derivative(Default)]
crate enum CmdBufferState {
    #[derivative(Default)]
    Initial,
    Recording,
    Executable,
    Pending,
}

#[derive(Debug)]
crate struct CmdPool {
    device: Arc<Device>,
    inner: vk::CommandPool,
    flags: vk::CommandPoolCreateFlags,
    queue_family: u32,
}

#[derive(Debug)]
crate struct CmdBuffer {
    device: Arc<Device>,
    inner: vk::CommandBuffer,
    level: CmdBufferLevel,
    pool: Box<CmdPool>,
    state: CmdBufferState,
}

#[derive(Debug)]
crate struct SubpassCmds {
    inner: CmdBuffer,
    framebuffer: Arc<Framebuffer>,
    subpass: Subpass,
    gfx_pipe: Option<Arc<GraphicsPipeline>>,
}

#[derive(Debug)]
crate struct RenderPassCmds {
    inner: CmdBuffer,
    framebuffer: Arc<Framebuffer>,
    cur_subpass: i32,
    cur_contents: SubpassContents,
}

#[derive(Clone, Copy, Debug, Derivative, Eq, Hash, PartialEq)]
#[derivative(Default)]
crate enum CmdBufferLevel {
    #[derivative(Default)]
    Primary,
    Secondary,
    SubpassContinue,
}

wrap_vk_enum! {
    #[derive(Derivative)]
    #[derivative(Default)]
    crate enum SubpassContents {
        #[derivative(Default)]
        Inline = INLINE,
        Secondary = SECONDARY_COMMAND_BUFFERS,
    }
}

// TODO: recorded buffers ought to increment a ref count on the command
// pool
impl Drop for CmdPool {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe { dt.destroy_command_pool(self.inner, ptr::null()); }
    }
}

impl CmdPool {
    crate fn new<'dev>(
        queue_family: QueueFamily<'dev>,
        flags: vk::CommandPoolCreateFlags,
    ) -> Self {
        use vk::CommandPoolCreateFlags as Flags;
        let allowed = Flags::TRANSIENT_BIT | Flags::RESET_COMMAND_BUFFER_BIT;
        assert!(allowed.contains(flags));

        let device = Arc::clone(queue_family.device());
        let dt = &*device.table;
        let create_info = vk::CommandPoolCreateInfo {
            flags,
            queue_family_index: queue_family.index(),
            ..Default::default()
        };
        let mut pool = vk::null();
        unsafe {
            dt.create_command_pool(&create_info, ptr::null(), &mut pool)
                .check().unwrap();
        }

        Self {
            device,
            inner: pool,
            flags,
            queue_family: queue_family.index(),
        }
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    crate fn is_transient(&self) -> bool {
        use vk::CommandPoolCreateFlags as Flags;
        self.flags.contains(Flags::TRANSIENT_BIT)
    }

    crate fn reset_enabled(&self) -> bool {
        use vk::CommandPoolCreateFlags as Flags;
        self.flags.contains(Flags::RESET_COMMAND_BUFFER_BIT)
    }

    crate fn queue_family(&self) -> QueueFamily<'_> {
        self.device.queue_family(self.queue_family)
    }

    crate fn supports_graphics(&self) -> bool {
        self.queue_family().supports_graphics()
    }

    crate fn alloc(&mut self, level: CmdBufferLevel) -> vk::CommandBuffer {
        trace!("allocating command buffer: queue_family: {}, {:?}, {:?}",
            self.queue_family, self.flags, level);
        let dt = &*self.device.table;
        let mut buffer = vk::null();
        let buffers = std::slice::from_mut(&mut buffer);
        let alloc_info = vk::CommandBufferAllocateInfo {
            command_pool: self.inner,
            level: level.into(),
            command_buffer_count: buffers.len() as _,
            ..Default::default()
        };
        unsafe {
            dt.allocate_command_buffers(&alloc_info, buffers.as_mut_ptr())
                .check().unwrap();
        }
        buffer
    }

    crate unsafe fn free(&mut self, cmds: &[vk::CommandBuffer]) {
        trace!("freeing command buffers: queue_family: {}, {:?}, count: {}",
            self.queue_family, self.flags, cmds.len());
        let dt = &*self.device.table;
        dt.free_command_buffers(self.inner, cmds.len() as _, cmds.as_ptr());
    }

    crate unsafe fn reset(&mut self) {
        let dt = &*self.device.table;
        dt.reset_command_pool(self.inner, Default::default());
    }
}

impl Drop for CmdBuffer {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            panic!("unused command buffer");
        }
    }
}

impl CmdBuffer {
    crate fn new(mut pool: Box<CmdPool>, level: CmdBufferLevel) -> Self {
        CmdBuffer {
            device: Arc::clone(&pool.device),
            inner: pool.alloc(level.into()),
            level,
            pool,
            state: Default::default(),
        }
    }

    fn ensure_recording(&self) {
        assert_eq!(self.state, CmdBufferState::Recording);
    }

    fn dt(&self) -> &vkl::DeviceTable {
        &self.device.table
    }

    crate fn inner(&self) -> vk::CommandBuffer {
        self.inner
    }

    crate fn state(&self) -> CmdBufferState {
        self.state
    }

    crate fn is_recording(&self) -> bool {
        self.state == CmdBufferState::Recording
    }

    crate fn level(&self) -> CmdBufferLevel {
        self.level
    }

    crate fn supports_graphics(&self) -> bool {
        self.pool.supports_graphics()
    }

    unsafe fn begin(
        &mut self,
        inheritance_info: Option<&vk::CommandBufferInheritanceInfo>,
    ) {
        let dt = &*self.device.table;

        assert_eq!(self.state, CmdBufferState::Initial);

        // TODO (eventually): reusable buffers
        let flags = self.level.required_usage_flags() |
            vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT_BIT;
        let begin_info = vk::CommandBufferBeginInfo {
            flags,
            p_inheritance_info: inheritance_info.as_ptr(),
            ..Default::default()
        };
        dt.begin_command_buffer(self.inner, &begin_info).check().unwrap();
        self.state = CmdBufferState::Recording;
    }

    unsafe fn do_end(&mut self) {
        let dt = &*self.device.table;
        assert_eq!(self.state, CmdBufferState::Recording);
        dt.end_command_buffer(self.inner).check().unwrap();
        self.state = CmdBufferState::Executable;
    }

    crate fn end(mut self) -> (vk::CommandBuffer, Box<CmdPool>) {
        unsafe {
            self.do_end();
            // Sadly, we must do this
            let _ = ptr::read(&self.device);
            let inner = self.inner;
            let pool = ptr::read(&self.pool);
            std::mem::forget(self);
            (inner, pool)
        }
    }

    // Shared command implementations

    unsafe fn set_viewport(&mut self, viewport: vk::Viewport) {
        debug_assert!(self.supports_graphics());
        let viewports = [viewport];
        self.dt().cmd_set_viewport(
            self.inner,
            0,
            viewports.len() as _,
            viewports.as_ptr(),
        );
    }

    unsafe fn set_scissor(&mut self, scissor: vk::Rect2D) {
        debug_assert!(self.supports_graphics());
        let scissors = [scissor];
        self.dt().cmd_set_scissor(
            self.inner,
            0,
            scissors.len() as _,
            scissors.as_ptr(),
        );
    }

    /// N.B.: values should be negative as depth buffer is reversed.
    // TODO: Depth clamping (maybe good for first-person rendering)
    unsafe fn set_depth_bias(
        &mut self,
        constant_factor: f32,
        slope_factor: f32,
    ) {
        debug_assert!(self.supports_graphics());
        self.dt().cmd_set_depth_bias(
            self.inner,
            constant_factor,
            0.0,
            slope_factor,
        );
    }

    unsafe fn reset_dynamic_state(&mut self, framebuffer: &Framebuffer) {
        self.set_viewport(framebuffer.viewport());
        self.set_scissor(framebuffer.render_area());
        // TODO: these numbers are somewhat arbitrary
        self.set_depth_bias(-0.005, -0.005);
    }
}

impl CmdBufferLevel {
    crate fn is_secondary(self) -> bool {
        use CmdBufferLevel::*;
        match self {
            Primary => false,
            Secondary | SubpassContinue => true,
        }
    }

    crate fn required_usage_flags(self) -> vk::CommandBufferUsageFlags {
        if self == Self::SubpassContinue {
            vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE_BIT
        } else { Default::default() }
    }
}

impl From<CmdBufferLevel> for vk::CommandBufferLevel {
    fn from(level: CmdBufferLevel) -> Self {
        if level.is_secondary() { Self::SECONDARY } else { Self::PRIMARY }
    }
}

impl SubpassCmds {
    crate unsafe fn secondary(
        framebuffer: Arc<Framebuffer>,
        subpass: Subpass,
        pool: Box<CmdPool>,
    ) -> Self {
        assert!(Arc::ptr_eq(subpass.pass(), framebuffer.pass()));
        let inner = CmdBuffer::new(pool, CmdBufferLevel::SubpassContinue);
        let mut cmds = SubpassCmds {
            inner,
            framebuffer,
            subpass,
            gfx_pipe: None,
        };
        cmds.begin_secondary();
        cmds
    }

    fn dt(&self) -> &vkl::DeviceTable {
        self.inner.dt()
    }

    fn ensure_recording(&self) {
        self.inner.ensure_recording();
    }

    crate fn raw(&self) -> vk::CommandBuffer {
        self.inner.inner()
    }

    crate fn state(&self) -> CmdBufferState {
        self.inner.state()
    }

    crate fn subpass(&self) -> &Subpass {
        &self.subpass
    }

    crate fn level(&self) -> CmdBufferLevel {
        self.inner.level
    }

    crate fn is_inline(&self) -> bool {
        self.inner.level != CmdBufferLevel::SubpassContinue
    }

    // Special initialization for secondary buffers
    unsafe fn begin_secondary(&mut self) {
        assert_eq!(self.inner.level(), CmdBufferLevel::SubpassContinue);
        assert_eq!(self.inner.state(), CmdBufferState::Initial);
        assert!(self.framebuffer.is_swapchain_valid());
        let inheritance_info = vk::CommandBufferInheritanceInfo {
            render_pass: self.subpass.pass().inner(),
            subpass: self.subpass.index(),
            framebuffer: self.framebuffer.inner(),
            ..Default::default()
        };
        self.inner.begin(Some(&inheritance_info));
        self.inner.reset_dynamic_state(&self.framebuffer);
    }

    crate fn bind_gfx_descs(
        &mut self,
        index: u32,
        set: &DescriptorSet,
    ) {
        self.ensure_recording();
        let bind_point = vk::PipelineBindPoint::GRAPHICS;
        let pipeline = self.gfx_pipe.as_ref().unwrap();
        let layout = &pipeline.layout();
        assert!(Arc::ptr_eq(
            set.layout(),
            &layout.set_layouts()[index as usize]
        ));
        let sets = [set.inner()];
        unsafe {
            self.dt().cmd_bind_descriptor_sets(
                self.raw(),         // commandBuffer
                bind_point,         // pipelineBindPoint
                layout.inner(),     // layout
                index,              // firstSet
                sets.len() as _,    // descriptorSetCount
                sets.as_ptr(),      // pDescriptorSets
                0,                  // dynamicOffsetCount
                ptr::null(),        // pDynamicOffsets
            );
        }
    }

    crate fn bind_gfx_pipe(&mut self, pipeline: &Arc<GraphicsPipeline>) {
        self.ensure_recording();
        try_opt! {
            if Arc::ptr_eq(self.gfx_pipe.as_ref()?, pipeline) {
                return;
            }
        };
        assert_eq!(&self.subpass, pipeline.subpass());
        unsafe {
            self.dt().cmd_bind_pipeline(
                self.raw(),
                vk::PipelineBindPoint::GRAPHICS,
                pipeline.inner(),
            );
        }
        self.gfx_pipe = Some(Arc::clone(pipeline));
    }

    crate unsafe fn bind_index_buffer<'a>(
        &mut self,
        buffer: BufferRange<'a>,
        ty: IndexType,
    ) {
        self.dt().cmd_bind_index_buffer(
            self.raw(),
            buffer.raw(),
            buffer.offset(),
            ty.into(),
        );
    }

    crate fn bind_vertex_buffers(&mut self, data: &VertexData<'_>) {
        let pipe = self.gfx_pipe.as_ref().unwrap();
        let layout = pipe.vertex_layout();

        let mut buffers = Vec::new();
        let mut offsets = Vec::new();
        for buffer in data.map_bindings(layout) {
            buffers.push(buffer.raw());
            offsets.push(buffer.offset());
        }
        assert!(!buffers.is_empty());

        unsafe {
            self.dt().cmd_bind_vertex_buffers(
                self.raw(),
                0,
                buffers.len() as _,
                buffers.as_ptr(),
                offsets.as_ptr(),
            );
        }
    }

    fn pre_draw(&mut self) {
        self.ensure_recording();
        // TODO: Check bound vertex buffer bounds (including instances)
        // TODO: Check bound descriptor sets
        assert!(self.gfx_pipe.is_some());
    }

    crate unsafe fn draw(&mut self, vertex_count: u32, instance_count: u32) {
        self.draw_offset(vertex_count, instance_count, 0, 0);
    }

    crate unsafe fn draw_offset(
        &mut self,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) {
        self.pre_draw();
        self.dt().cmd_draw(
            self.raw(),
            vertex_count, instance_count,
            first_vertex, first_instance,
        );
    }

    crate unsafe fn draw_indexed(
        &mut self,
        vertex_count: u32,
        instance_count: u32,
    ) {
        self.draw_indexed_offset(vertex_count, instance_count, 0, 0, 0);
    }

    crate unsafe fn draw_indexed_offset(
        &mut self,
        vertex_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
    ) {
        self.pre_draw();
        self.dt().cmd_draw_indexed(
            self.raw(),
            vertex_count,
            instance_count,
            first_index,
            vertex_offset,
            first_instance,
        );
    }

    /// Stops recording commands within the current subpass. Does *not*
    /// advance to the next subpass.
    crate fn exit_subpass(self) -> RenderPassCmds {
        self.ensure_recording();
        assert!(self.is_inline());
        RenderPassCmds {
            inner: self.inner,
            framebuffer: self.framebuffer,
            cur_subpass: self.subpass.index() as _,
            cur_contents: SubpassContents::Inline,
        }
    }

    /// Ends recording of a secondary render pass continuation.
    crate fn end_secondary(self) -> (vk::CommandBuffer, Box<CmdPool>) {
        self.ensure_recording();
        assert!(!self.is_inline());
        self.inner.end()
    }

    crate unsafe fn set_viewport(&mut self, viewport: vk::Viewport) {
        self.inner.set_viewport(viewport);
    }

    crate unsafe fn set_scissors(&mut self, scissor: vk::Rect2D) {
        self.inner.set_scissor(scissor);
    }

    crate fn set_depth_bias(
        &mut self,
        constant_factor: f32,
        slope_factor: f32,
    ) {
        unsafe { self.inner.set_depth_bias(constant_factor, slope_factor); }
    }
}

impl RenderPassCmds {
    crate fn new(
        cmds: CmdBuffer,
        framebuffer: Arc<Framebuffer>,
        clear_values: &[vk::ClearValue],
        contents: SubpassContents,
    ) -> Self {
        assert!(cmds.supports_graphics());
        assert_ne!(cmds.level, CmdBufferLevel::SubpassContinue);
        let mut cmds = RenderPassCmds {
            inner: cmds,
            framebuffer,
            cur_subpass: -1,
            cur_contents: Default::default(),
        };
        unsafe { cmds.begin(clear_values, contents); }
        cmds
    }

    fn dt(&self) -> &vkl::DeviceTable {
        self.inner.dt()
    }

    crate fn framebuffer(&self) -> &Arc<Framebuffer> {
        &self.framebuffer
    }

    crate fn pass(&self) -> &Arc<RenderPass> {
        &self.framebuffer.pass()
    }

    crate fn raw(&self) -> vk::CommandBuffer {
        self.inner.inner()
    }

    crate fn state(&self) -> CmdBufferState {
        self.inner.state()
    }

    crate fn level(&self) -> CmdBufferLevel {
        self.inner.level
    }

    crate fn cur_subpass(&self) -> Subpass {
        self.framebuffer.pass().subpass(self.cur_subpass as _)
    }

    fn check_state(&self) {
        let subpass_count = self.pass().subpasses().len();
        assert!((self.cur_subpass as usize) < subpass_count);
        if self.level() == CmdBufferLevel::Secondary {
            assert_eq!(self.cur_contents, SubpassContents::Inline);
        }
    }

    unsafe fn begin(
        &mut self,
        clear_values: &[vk::ClearValue],
        contents: SubpassContents,
    ) {
        assert!(self.cur_subpass < 0);
        self.cur_subpass = 0;
        self.cur_contents = contents;
        self.check_state();

        if !self.inner.is_recording() {
            self.inner.begin(None);
        }
        self.inner.reset_dynamic_state(&self.framebuffer);

        assert!(self.framebuffer.is_swapchain_valid());

        // TODO: Clear color
        let begin_info = vk::RenderPassBeginInfo {
            render_pass: self.framebuffer.pass().inner(),
            framebuffer: self.framebuffer.inner(),
            render_area: self.framebuffer.render_area(),
            clear_value_count: clear_values.len() as _,
            p_clear_values: clear_values.as_ptr(),
            ..Default::default()
        };
        self.dt().cmd_begin_render_pass(
            self.raw(),
            &begin_info,
            contents.into(),
        );
    }

    fn ensure_recording(&self) {
        // Should be guaranteed by constructor
        debug_assert_eq!(self.inner.state, CmdBufferState::Recording);
        debug_assert!(self.cur_subpass >= 0);
    }

    crate fn enter_subpass(self) -> SubpassCmds {
        self.ensure_recording();
        assert_eq!(self.cur_contents, SubpassContents::Inline);
        let subpass = self.cur_subpass();
        SubpassCmds {
            inner: self.inner,
            framebuffer: self.framebuffer,
            subpass,
            gfx_pipe: None,
        }
    }

    crate fn next_subpass(&mut self, contents: SubpassContents) {
        self.ensure_recording();
        self.cur_subpass += 1;
        self.cur_contents = contents;
        self.check_state();
        unsafe { self.dt().cmd_next_subpass(self.raw(), contents.into()); }
    }

    crate unsafe fn execute_cmds(&mut self, cmds: &[vk::CommandBuffer]) {
        assert_eq!(self.level(), CmdBufferLevel::Primary);
        assert_eq!(self.cur_contents, SubpassContents::Secondary);
        self.dt().cmd_execute_commands(
            self.raw(),
            cmds.len() as _,
            cmds.as_ptr(),
        );
    }

    crate fn end(self) -> CmdBuffer {
        unsafe { self.dt().cmd_end_render_pass(self.raw()); }
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use super::*;

    unsafe fn test_common(vars: &testing::TestVars) -> (
        SystemState, Arc<Globals>, TrivialRenderer, TrivialPass,
        Vec<Arc<Framebuffer>>, Box<CmdPool>,
    ) {
        let device = Arc::clone(vars.device());

        let state = SystemState::new(Arc::clone(&device));
        let globals = Arc::new(Globals::new(&state));
        let trivial = TrivialRenderer::new(&state, Arc::clone(&globals));

        let pass = TrivialPass::new(Arc::clone(&device));
        let framebuffers = pass.create_framebuffers(&vars.swapchain);

        let pool = Box::new(CmdPool::new(
            vars.gfx_queue().family(),
            vk::CommandPoolCreateFlags::TRANSIENT_BIT,
        ));
        (state, globals, trivial, pass, framebuffers, pool)
    }

    unsafe fn record_subpass(
        state: &SystemState,
        globals: &Globals,
        trivial: &TrivialRenderer,
        cmds: &mut SubpassCmds,
    ) {
        let mut desc = GraphicsPipelineDesc::new(
            cmds.subpass().clone(),
            Arc::clone(&trivial.pipeline_layout()),
        );

        let shaders = &globals.shaders;
        desc.stages[ShaderStage::Vertex] =
            Some(Arc::new(Arc::clone(&shaders.trivial_vert).into()));
        desc.stages[ShaderStage::Fragment] =
            Some(Arc::new(Arc::clone(&shaders.trivial_frag).into()));

        let pipe = state.gfx_pipes.get_or_create(&desc);
        cmds.bind_gfx_pipe(&pipe);

        cmds.bind_gfx_descs(0, &trivial.descriptors()[0]);
        cmds.bind_gfx_descs(1, &trivial.descriptors()[1]);

        cmds.draw(TrivialRenderer::vertex_count(), 1);
    }

    unsafe fn subpass_test(vars: testing::TestVars) {
        let (state, globals, trivial, pass, framebuffers, pool) =
            test_common(&vars);
        let mut cmds = SubpassCmds::secondary(
            Arc::clone(&framebuffers[0]), pass.subpass.clone(), pool);
        record_subpass(&state, &globals, &trivial, &mut cmds);
        let (_, _) = cmds.end_secondary();
    }

    unsafe fn render_pass_test(vars: testing::TestVars) {
        // TODO: Test next_subpass()
        let (state, globals, trivial, _, framebuffers, pool) =
            test_common(&vars);
        let mut cmds = RenderPassCmds::new(
            CmdBuffer::new(pool, CmdBufferLevel::Primary),
            Arc::clone(&framebuffers[0]),
            SubpassContents::Inline,
        ).enter_subpass();
        record_subpass(&state, &globals, &trivial, &mut cmds);
        let (_, _) = cmds.exit_subpass().end().end();
    }

    unsafe fn subpass_out_of_bounds(vars: testing::TestVars) {
        let (_a, _b, _c, _d, framebuffers, pool) = test_common(&vars);
        let mut cmds = RenderPassCmds::new(
            CmdBuffer::new(pool, CmdBufferLevel::Primary),
            Arc::clone(&framebuffers[0]),
            SubpassContents::Inline,
        );
        cmds.next_subpass(SubpassContents::Inline);
    }

    unsafe fn inline_in_secondary_subpass(vars: testing::TestVars) {
        let (_a, _b, _c, _d, framebuffers, pool) = test_common(&vars);
        let cmds = RenderPassCmds::new(
            CmdBuffer::new(pool, CmdBufferLevel::Primary),
            Arc::clone(&framebuffers[0]),
            SubpassContents::Secondary,
        );
        cmds.enter_subpass();
    }

    unsafe fn exec_in_inline_subpass(vars: testing::TestVars) {
        let (_a, _b, _c, _d, framebuffers, pool) = test_common(&vars);
        let mut cmds = RenderPassCmds::new(
            CmdBuffer::new(pool, CmdBufferLevel::Primary),
            Arc::clone(&framebuffers[0]),
            SubpassContents::Inline,
        );
        cmds.execute_cmds(&[vk::null()]);
    }

    unit::declare_tests![
        subpass_test,
        render_pass_test,
        (#[should_err] subpass_out_of_bounds),
        (#[should_err] inline_in_secondary_subpass),
        (#[should_err] exec_in_inline_subpass),
    ];
}

unit::collect_tests![tests];
