use std::ptr;
use std::sync::Arc;

use derivative::Derivative;
use log::trace;
use prelude::*;

use crate::*;

#[derive(Clone, Copy, Debug, Derivative, Eq, Hash, PartialEq)]
enum CmdBufferState {
    Initial,
    Recording,
    Executable,
    Pending,
}

// TODO: Keep track of how many live command buffer objects there are;
// it's too easy to leak command buffers.
#[derive(Debug)]
crate struct CmdPool {
    device: Arc<Device>,
    inner: vk::CommandPool,
    flags: vk::CommandPoolCreateFlags,
    queue_family: u32,
    name: Option<String>,
}

// TODO: Should probably contain Arc<SystemState>
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

#[derive(Debug)]
crate struct XferCmds {
    inner: CmdBuffer,
}

#[derive(Clone, Copy, Debug, Derivative, Eq, Hash, PartialEq)]
#[derivative(Default)]
crate enum CmdBufferLevel {
    #[derivative(Default)]
    Primary,
    Secondary,
    /// Equivalent to secondary + RENDER_PASS_CONTINUE_BIT.
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
    crate fn new(
        queue_family: QueueFamily<'_>,
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
            name: None,
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

    crate fn supports_xfer(&self) -> bool {
        self.queue_family().supports_xfer()
    }

    crate fn alloc(&mut self, level: CmdBufferLevel) -> vk::CommandBuffer {
        trace!("CmdPool::alloc(self: {:?}, level: {:?})",
            fmt_named(&*self), level);
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
        trace!("CmdPool::free(self: {:?}, queue_family: {}, cmds: {:?})",
            fmt_named(&*self), self.queue_family, cmds);
        let dt = &*self.device.table;
        dt.free_command_buffers(self.inner, cmds.len() as _, cmds.as_ptr());
    }

    crate unsafe fn reset(&mut self) {
        let dt = &*self.device.table;
        dt.reset_command_pool(self.inner, Default::default());
    }

    crate fn set_name(&mut self, name: impl Into<String>) {
        let name: String = name.into();
        self.name = Some(name.clone());
        unsafe { self.device().set_name(self.inner, name); }
    }
}

impl Named for CmdPool {
    fn name(&self) -> Option<&str> {
        Some(&self.name.as_ref()?)
    }
}

impl Drop for CmdBuffer {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            panic!("unused command buffer");
        }
    }
}

// TODO: This API hardly supports reusing command buffers.
impl CmdBuffer {
    crate fn new(mut pool: Box<CmdPool>, level: CmdBufferLevel) -> Self {
        Self {
            device: Arc::clone(pool.device()),
            inner: pool.alloc(level),
            level,
            pool,
            state: CmdBufferState::Initial,
        }
    }

    crate fn new_primary(pool: Box<CmdPool>) -> Self {
        Self::new(pool, CmdBufferLevel::Primary)
    }

    /// Creates a command buffer from a raw Vulkan command buffer
    /// handle. The underlying command buffer object *must* be in the
    /// initial state.
    crate unsafe fn from_initial(
        pool: Box<CmdPool>,
        cmds: vk::CommandBuffer,
        level: CmdBufferLevel,
    ) -> Self {
        Self {
            device: Arc::clone(pool.device()),
            inner: cmds,
            pool,
            level,
            state: CmdBufferState::Initial,
        }
    }

    crate fn device(&self) -> &Arc<Device> {
        self.pool.device()
    }

    fn dt(&self) -> &vkl::DeviceTable {
        &self.device().table
    }

    crate fn inner(&self) -> vk::CommandBuffer {
        self.inner
    }

    fn state(&self) -> CmdBufferState {
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

    crate fn supports_xfer(&self) -> bool {
        self.pool.supports_xfer()
    }

    fn ensure_recording(&self) {
        assert_eq!(self.state, CmdBufferState::Recording);
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

    crate unsafe fn do_end(&mut self) {
        let dt = &*self.device.table;
        self.ensure_recording();
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

    // TODO: Allow binding multiple in one call
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
        tryopt! {
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
        // TODO: should be able to use an already begun command buffer
        // if requisites are met
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

impl XferCmds {
    crate fn new(mut cmds: CmdBuffer) -> Self {
        assert!(cmds.supports_xfer());
        assert_ne!(cmds.level, CmdBufferLevel::SubpassContinue);
        unsafe { cmds.begin(None); }
        Self { inner: cmds }
    }

    fn dt(&self) -> &vkl::DeviceTable {
        self.inner.dt()
    }

    crate fn raw(&self) -> vk::CommandBuffer {
        self.inner.inner()
    }

    crate unsafe fn pipeline_barrier(
        &mut self,
        src_stage_mask: vk::PipelineStageFlags,
        dst_stage_mask: vk::PipelineStageFlags,
        dependency_flags: vk::DependencyFlags,
        global_barriers: &[vk::MemoryBarrier],
        buffer_barriers: &[vk::BufferMemoryBarrier],
        image_barriers: &[vk::ImageMemoryBarrier],
    ) {
        self.dt().cmd_pipeline_barrier(
            self.raw(),
            src_stage_mask,
            dst_stage_mask,
            dependency_flags,
            global_barriers.len() as _,
            global_barriers.as_ptr(),
            buffer_barriers.len() as _,
            buffer_barriers.as_ptr(),
            image_barriers.len() as _,
            image_barriers.as_ptr(),
        );
    }

    // TODO: Could take an iterator over BufferRange pairs
    crate unsafe fn copy_buffer(
        &mut self,
        src: &Arc<DeviceBuffer>,
        dst: &Arc<DeviceBuffer>,
        regions: &[vk::BufferCopy],
    ) {
        // This check is good for catching unnecessary copies on UMA.
        // However, there are use cases that may need to be allowed.
        assert!(!Arc::ptr_eq(src, dst),
            "copy to same buffer (likely unintended)");
        for region in regions.iter() {
            assert!(region.src_offset + region.size <= src.size());
            assert!(region.dst_offset + region.size <= dst.size());
        }
        self.dt().cmd_copy_buffer(
            self.raw(),
            src.inner(),
            dst.inner(),
            regions.len() as _,
            regions.as_ptr(),
        )
    }

    crate unsafe fn copy_buffer_to_image(
        &mut self,
        src: &DeviceBuffer,
        dst: &Arc<Image>,
        layout: vk::ImageLayout,
        regions: &[vk::BufferImageCopy],
    ) {
        trace!(concat!(
            "XferCmds::copy_buffer_to_image(src: {:?}, dst: {:?}, ",
            "layout: {:?}, regions: {:?})",
        ), fmt_named(&*src), fmt_named(&**dst), layout, regions);
        validate_buffer_image_copy(src, dst, layout, regions);
        self.dt().cmd_copy_buffer_to_image(
            self.raw(),
            src.inner(),
            dst.inner(),
            layout,
            regions.len() as _,
            regions.as_ptr(),
        );
    }

    crate fn end_xfer(self) -> CmdBuffer {
        self.inner
    }

    crate fn end(self) -> (vk::CommandBuffer, Box<CmdPool>) {
        self.inner.end()
    }
}

#[cfg(debug_assertions)]
fn validate_buffer_image_copy(
    src: &DeviceBuffer,
    dst: &Image,
    layout: vk::ImageLayout,
    regions: &[vk::BufferImageCopy],
) {
    use math::vector::Vector3;

    assert!([
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        vk::ImageLayout::GENERAL,
        vk::ImageLayout::SHARED_PRESENT_KHR,
    ].contains(&layout));
    for region in regions.iter() {
        let off = Vector3::from(<(i32, i32, i32)>::from(region.image_offset));
        let ext = Extent3D::from(region.image_extent);
        assert!(dst.extent().contains_extent(off, ext));

        let texel_size = dst.format().size();
        let row_length = if region.buffer_row_length == 0 {
            region.buffer_row_length
        } else { region.image_extent.width } as usize;
        let image_height = if region.buffer_image_height == 0 {
            region.buffer_image_height
        } else { region.image_extent.height } as usize;
        let layer_texels = row_length * image_height;
        let layer_count = region.image_subresource.layer_count as usize;
        let size = (layer_count * layer_texels * texel_size) as vk::DeviceSize;
        assert!(region.buffer_offset + size < src.size());
    }
}

#[cfg(not(debug_assertions))]
fn validate_buffer_image_copy(
    _: &DeviceBuffer,
    _: &Image,
    _: vk::ImageLayout,
    _: &[vk::BufferImageCopy],
) {
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use super::*;

    unsafe fn test_common(vars: &testing::TestVars) -> (
        SystemState, TrivialRenderer, TrivialPass, Vec<Arc<Framebuffer>>,
        Box<CmdPool>,
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
        (state, trivial, pass, framebuffers, pool)
    }

    unsafe fn record_subpass(vars: testing::TestVars) {
        let (state, trivial, pass, framebuffers, pool) = test_common(&vars);
        let mut cmds = SubpassCmds::secondary(
            Arc::clone(&framebuffers[0]), pass.subpass.clone(), pool);
        trivial.render(&state, &mut cmds);
        let (_, _) = cmds.end_secondary();
    }

    unsafe fn record_render_pass(vars: testing::TestVars) {
        // TODO: Test next_subpass()
        let (state, trivial, _, framebuffers, pool) = test_common(&vars);
        let mut cmds = RenderPassCmds::new(
            CmdBuffer::new(pool, CmdBufferLevel::Primary),
            Arc::clone(&framebuffers[0]),
            &[],
            SubpassContents::Inline,
        ).enter_subpass();
        trivial.render(&state, &mut cmds);
        let (_, _) = cmds.exit_subpass().end().end();
    }

    unsafe fn subpass_out_of_bounds(vars: testing::TestVars) {
        let (_a, _b, _c, framebuffers, pool) = test_common(&vars);
        let mut cmds = RenderPassCmds::new(
            CmdBuffer::new(pool, CmdBufferLevel::Primary),
            Arc::clone(&framebuffers[0]),
            &[],
            SubpassContents::Inline,
        );
        cmds.next_subpass(SubpassContents::Inline);
    }

    unsafe fn inline_in_secondary_subpass(vars: testing::TestVars) {
        let (_a, _b, _c, framebuffers, pool) = test_common(&vars);
        let cmds = RenderPassCmds::new(
            CmdBuffer::new(pool, CmdBufferLevel::Primary),
            Arc::clone(&framebuffers[0]),
            &[],
            SubpassContents::Secondary,
        );
        cmds.enter_subpass();
    }

    unsafe fn exec_in_inline_subpass(vars: testing::TestVars) {
        let (_a, _b, _c, framebuffers, pool) = test_common(&vars);
        let mut cmds = RenderPassCmds::new(
            CmdBuffer::new(pool, CmdBufferLevel::Primary),
            Arc::clone(&framebuffers[0]),
            &[],
            SubpassContents::Inline,
        );
        cmds.execute_cmds(&[vk::null()]);
    }

    unsafe fn copy_common(vars: &testing::TestVars) -> (SystemState, XferCmds)
    {
        let device = Arc::clone(vars.device());
        let state = SystemState::new(Arc::clone(&device));
        let pool = Box::new(CmdPool::new(
            vars.gfx_queue().family(),
            vk::CommandPoolCreateFlags::TRANSIENT_BIT,
        ));
        let cmds = CmdBuffer::new(pool, CmdBufferLevel::Primary);
        (state, XferCmds::new(cmds))
    }

    unsafe fn copy_buffer(vars: testing::TestVars) {
        let (state, mut cmds) = copy_common(&vars);
        let src = state.buffers.alloc(
            BufferBinding::Storage,
            Lifetime::Frame,
            MemoryMapping::Mapped,
            1024,
        );
        let dst = state.buffers.alloc(
            BufferBinding::Vertex,
            Lifetime::Frame,
            MemoryMapping::DeviceLocal,
            1024,
        );
        cmds.copy_buffer(src.buffer(), dst.buffer(), &[
            vk::BufferCopy {
                src_offset: 0,
                dst_offset: 0,
                size: 512,
            },
            vk::BufferCopy {
                src_offset: 512,
                dst_offset: 768,
                size: 256,
            },
        ]);
        cmds.end_xfer().end();
    }

    unsafe fn copy_intra_buffer(vars: testing::TestVars) {
        let (state, mut cmds) = copy_common(&vars);
        let buf = state.buffers.alloc(
            BufferBinding::Storage,
            Lifetime::Frame,
            MemoryMapping::Mapped,
            1024,
        );
        cmds.copy_buffer(buf.buffer(), buf.buffer(), &[
            vk::BufferCopy {
                src_offset: 0,
                dst_offset: 1536,
                size: 512,
            },
            vk::BufferCopy {
                src_offset: 512,
                dst_offset: 1024,
                size: 512,
            },
        ]);
        cmds.end_xfer().end();
    }

    unsafe fn copy_image(vars: testing::TestVars) {
        let (state, mut cmds) = copy_common(&vars);
        let format = Format::RGBA8;
        let src = state.buffers.alloc(
            BufferBinding::Storage,
            Lifetime::Frame,
            MemoryMapping::Mapped,
            (64 * 64 * format.size()) as _,
        );
        let dst = Arc::new(Image::with(
            &state.heap,
            ImageFlags::NO_SAMPLE,
            ImageType::Dim2,
            format,
            SampleCount::One,
            Extent3D::new(64, 64, 1),
            1,
            1,
        ));
        cmds.copy_buffer_to_image(
            src.buffer(),
            &dst,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[vk::BufferImageCopy {
                image_subresource: dst.all_layers_for_mip_level(0)
                    .to_mip_layers(0),
                image_extent: dst.extent().into(),
                ..Default::default()
            }],
        );
        cmds.end_xfer().end();
    }

    unit::declare_tests![
        record_subpass,
        record_render_pass,
        (#[should_err] subpass_out_of_bounds),
        (#[should_err] inline_in_secondary_subpass),
        (#[should_err] exec_in_inline_subpass),
        copy_buffer,
        (#[should_err] copy_intra_buffer),
        copy_image,
    ];
}

unit::collect_tests![tests];
