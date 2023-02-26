#![allow(clippy::borrowed_box)]

use std::ptr;
use std::sync::Arc;

use derivative::Derivative;
use log::trace;

use crate::*;

#[derive(Clone, Copy, Debug, Derivative, Eq, Hash, PartialEq)]
enum CmdBufferState {
    Initial,
    Recording,
    Executable,
}

// TODO: Keep track of how many live command buffer objects there are;
// it's too easy to leak command buffers.
#[derive(Debug)]
pub struct CmdPool {
    device: Arc<Device>,
    inner: vk::CommandPool,
    flags: vk::CommandPoolCreateFlags,
    queue_family: u32,
    name: Option<String>,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct CmdBuffer {
    device: Arc<Device>,
    inner: vk::CommandBuffer,
    level: CmdBufferLevel,
    #[derivative(Debug(format_with = "write_named::<CmdPool>"))]
    pool: Box<CmdPool>,
    state: CmdBufferState,
    framebuffer: Option<Arc<Framebuffer>>,
    cur_subpass: u32,
    cur_contents: SubpassContents,
    gfx_pipe: Option<Arc<GraphicsPipeline>>,
}

#[derive(Clone, Copy, Debug, Derivative, Eq, Hash, PartialEq)]
#[derivative(Default)]
pub enum CmdBufferLevel {
    #[derivative(Default)]
    Primary,
    Secondary,
    /// Equivalent to secondary + RENDER_PASS_CONTINUE_BIT.
    SubpassContinue,
}

wrap_vk_enum! {
    #[derive(Derivative)]
    #[derivative(Default)]
    pub enum SubpassContents {
        #[derivative(Default)]
        Inline = INLINE,
        Secondary = SECONDARY_COMMAND_BUFFERS,
    }
}

impl CmdBufferLevel {
    #[inline]
    pub fn is_secondary(self) -> bool {
        use CmdBufferLevel::*;
        match self {
            Primary => false,
            Secondary | SubpassContinue => true,
        }
    }

    #[inline]
    pub fn required_usage_flags(self) -> vk::CommandBufferUsageFlags {
        if self == Self::SubpassContinue {
            vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE_BIT
        } else {
            Default::default()
        }
    }
}

impl From<CmdBufferLevel> for vk::CommandBufferLevel {
    fn from(level: CmdBufferLevel) -> Self {
        if level.is_secondary() {
            Self::SECONDARY
        } else {
            Self::PRIMARY
        }
    }
}

// TODO: recorded buffers ought to increment a ref count on the command
// pool
impl Drop for CmdPool {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            dt.destroy_command_pool(self.inner, ptr::null());
        }
    }
}

impl CmdPool {
    pub fn new(queue_family: QueueFamily<'_>, flags: vk::CommandPoolCreateFlags) -> Self {
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
                .check()
                .unwrap();
        }

        Self {
            device,
            inner: pool,
            flags,
            queue_family: queue_family.index(),
            name: None,
        }
    }

    pub fn new_transient(queue_family: QueueFamily<'_>) -> Self {
        Self::new(queue_family, vk::CommandPoolCreateFlags::TRANSIENT_BIT)
    }

    #[inline]
    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    #[inline]
    pub fn is_transient(&self) -> bool {
        use vk::CommandPoolCreateFlags as Flags;
        self.flags.contains(Flags::TRANSIENT_BIT)
    }

    #[inline]
    pub fn reset_enabled(&self) -> bool {
        use vk::CommandPoolCreateFlags as Flags;
        self.flags.contains(Flags::RESET_COMMAND_BUFFER_BIT)
    }

    #[inline]
    pub fn queue_family(&self) -> QueueFamily<'_> {
        self.device.queue_family(self.queue_family)
    }

    #[inline]
    pub fn supports_graphics(&self) -> bool {
        self.queue_family().supports_graphics()
    }

    #[inline]
    pub fn supports_xfer(&self) -> bool {
        self.queue_family().supports_xfer()
    }

    pub fn alloc(&mut self, level: CmdBufferLevel) -> vk::CommandBuffer {
        trace!(
            "CmdPool::alloc(self: {:?}, level: {:?})",
            fmt_named(&*self),
            level
        );
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
                .check()
                .unwrap();
        }
        buffer
    }

    pub unsafe fn free(&mut self, cmds: &[vk::CommandBuffer]) {
        trace!(
            "CmdPool::free(self: {:?}, queue_family: {}, cmds: {:?})",
            fmt_named(&*self),
            self.queue_family,
            cmds
        );
        let dt = &*self.device.table;
        dt.free_command_buffers(self.inner, cmds.len() as _, cmds.as_ptr());
    }

    pub unsafe fn reset(&mut self) {
        let dt = &*self.device.table;
        dt.reset_command_pool(self.inner, Default::default());
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        let name: String = name.into();
        self.name = Some(name.clone());
        unsafe {
            self.device().set_name(self.inner, name);
        }
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

impl CmdBuffer {
    pub fn new(mut pool: Box<CmdPool>) -> Self {
        let level = CmdBufferLevel::Primary;
        let inner = pool.alloc(level);
        unsafe { Self::from_initial(pool, inner, level) }
    }

    pub fn new_secondary(mut pool: Box<CmdPool>) -> Self {
        let level = CmdBufferLevel::Secondary;
        let inner = pool.alloc(level);
        unsafe { Self::from_initial(pool, inner, level) }
    }

    pub fn new_subpass(
        mut pool: Box<CmdPool>,
        framebuffer: Arc<Framebuffer>,
        subpass: u32,
    ) -> Self {
        let level = CmdBufferLevel::SubpassContinue;
        let inner = pool.alloc(level);
        let mut cmds = unsafe { Self::from_initial(pool, inner, level) };
        cmds.framebuffer = Some(framebuffer);
        cmds.cur_subpass = subpass;
        cmds
    }

    /// Creates a command buffer from a raw Vulkan command buffer
    /// handle. The underlying command buffer object *must* be in the
    /// initial state.
    pub unsafe fn from_initial(
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
            framebuffer: Default::default(),
            cur_subpass: 0,
            cur_contents: Default::default(),
            gfx_pipe: None,
        }
    }

    #[inline]
    pub fn device(&self) -> &Arc<Device> {
        self.pool.device()
    }

    fn dt(&self) -> &vkl::DeviceTable {
        &self.device().table
    }

    #[inline]
    pub fn inner(&self) -> vk::CommandBuffer {
        self.inner
    }

    fn state(&self) -> CmdBufferState {
        self.state
    }

    #[inline]
    pub fn is_recording(&self) -> bool {
        self.state == CmdBufferState::Recording
    }

    #[inline]
    pub fn level(&self) -> CmdBufferLevel {
        self.level
    }

    #[inline]
    pub fn supports_graphics(&self) -> bool {
        self.pool.supports_graphics()
    }

    #[inline]
    pub fn supports_xfer(&self) -> bool {
        self.pool.supports_xfer()
    }

    #[inline]
    pub fn framebuffer(&self) -> Option<&Arc<Framebuffer>> {
        self.framebuffer.as_ref()
    }

    #[inline]
    pub fn render_pass(&self) -> Option<&Arc<RenderPass>> {
        Some(self.framebuffer.as_ref()?.render_pass())
    }

    #[inline]
    pub fn raw(&self) -> vk::CommandBuffer {
        self.inner()
    }

    #[inline]
    pub fn subpass(&self) -> Option<Subpass> {
        Some(Subpass {
            pass: Arc::clone(self.render_pass().as_ref()?),
            index: self.cur_subpass,
        })
    }

    #[inline]
    pub fn is_inline(&self) -> bool {
        self.level != CmdBufferLevel::SubpassContinue
    }

    #[inline]
    pub fn cur_subpass(&self) -> Option<Subpass> {
        Some(
            self.framebuffer
                .as_ref()?
                .render_pass()
                .subpass(self.cur_subpass as _),
        )
    }

    fn ensure_recording(&self) {
        assert_eq!(self.state, CmdBufferState::Recording);
    }

    unsafe fn begin_inner(&mut self, inheritance_info: Option<&vk::CommandBufferInheritanceInfo>) {
        trace!(
            "CmdBuffer::begin(self: {:?}, inheritance_info: {:?})",
            self,
            inheritance_info
        );

        let dt = &*self.device.table;

        assert_eq!(self.state, CmdBufferState::Initial);

        // TODO (eventually): reusable buffers
        let flags =
            self.level.required_usage_flags() | vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT_BIT;
        let begin_info = vk::CommandBufferBeginInfo {
            flags,
            p_inheritance_info: inheritance_info.as_ptr(),
            ..Default::default()
        };
        dt.begin_command_buffer(self.inner, &begin_info)
            .check()
            .unwrap();
        self.state = CmdBufferState::Recording;
    }

    fn begin_subpass_continue(&mut self) {
        assert_eq!(self.level(), CmdBufferLevel::SubpassContinue);
        assert_eq!(self.state(), CmdBufferState::Initial);
        assert!(self.framebuffer().unwrap().is_swapchain_valid());
        let render_pass = self.render_pass().unwrap();
        let inheritance_info = vk::CommandBufferInheritanceInfo {
            render_pass: render_pass.inner(),
            subpass: self.cur_subpass,
            framebuffer: self.framebuffer.as_ref().unwrap().inner(),
            ..Default::default()
        };
        unsafe {
            self.begin_inner(Some(&inheritance_info));
        }
        self.reset_dynamic_state();
    }

    pub fn begin(&mut self) {
        if self.state != CmdBufferState::Initial {
            return;
        }
        if self.level() == CmdBufferLevel::SubpassContinue {
            self.begin_subpass_continue();
        } else {
            unsafe {
                self.begin_inner(None);
            }
        }
    }

    pub unsafe fn do_end(&mut self) {
        trace!("CmdBuffer::end(self.inner: {:?})", self.inner);
        let dt = &*self.device.table;
        self.ensure_recording();
        dt.end_command_buffer(self.inner).check().unwrap();
        self.state = CmdBufferState::Executable;
    }

    // TODO: Create a proper abstraction around finished command buffers
    pub fn end(mut self) -> (vk::CommandBuffer, Box<CmdPool>) {
        unsafe {
            if self.framebuffer.is_some() && self.level == CmdBufferLevel::Primary {
                self.end_render_pass();
            }
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

    pub fn set_viewport(&mut self, viewport: vk::Viewport) {
        debug_assert!(self.supports_graphics());
        let viewports = [viewport];
        unsafe {
            self.dt()
                .cmd_set_viewport(self.inner, 0, viewports.len() as _, viewports.as_ptr());
        }
    }

    pub fn set_scissor(&mut self, scissor: vk::Rect2D) {
        debug_assert!(self.supports_graphics());
        let scissors = [scissor];
        unsafe {
            self.dt()
                .cmd_set_scissor(self.inner, 0, scissors.len() as _, scissors.as_ptr());
        }
    }

    /// N.B.: values should be negative as depth buffer is reversed.
    // TODO: Depth clamping (maybe good for first-person rendering)
    pub fn set_depth_bias(&mut self, constant_factor: f32, slope_factor: f32) {
        debug_assert!(self.supports_graphics());
        unsafe {
            self.dt()
                .cmd_set_depth_bias(self.inner, constant_factor, 0.0, slope_factor);
        }
    }

    pub fn reset_dynamic_state(&mut self) {
        self.set_viewport(self.framebuffer().unwrap().viewport());
        self.set_scissor(self.framebuffer().unwrap().render_area());
        // TODO: these numbers are somewhat arbitrary
        self.set_depth_bias(-0.005, -0.005);
    }

    pub fn bind_gfx_descs(&mut self, index: u32, set: &DescriptorSet) {
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
                self.raw(),      // commandBuffer
                bind_point,      // pipelineBindPoint
                layout.inner(),  // layout
                index,           // firstSet
                sets.len() as _, // descriptorSetCount
                sets.as_ptr(),   // pDescriptorSets
                0,               // dynamicOffsetCount
                ptr::null(),     // pDynamicOffsets
            );
        }
    }

    pub fn bind_gfx_pipe(&mut self, pipeline: &Arc<GraphicsPipeline>) {
        tryopt! {
            if Arc::ptr_eq(self.gfx_pipe.as_ref()?, pipeline) {
                return;
            }
        };
        self.ensure_recording();
        assert!(self.framebuffer.is_some());
        assert_eq!(&self.subpass().unwrap(), pipeline.subpass());
        unsafe {
            self.dt().cmd_bind_pipeline(
                self.raw(),
                vk::PipelineBindPoint::GRAPHICS,
                pipeline.inner(),
            );
        }
        self.gfx_pipe = Some(Arc::clone(pipeline));
    }

    pub fn bind_index_buffer(&mut self, buffer: BufferRange<'_>, ty: IndexType) {
        unsafe {
            self.dt()
                .cmd_bind_index_buffer(self.raw(), buffer.raw(), buffer.offset(), ty.into());
        }
    }

    pub fn bind_vertex_buffers<'a>(&mut self, buffers: impl IntoIterator<Item = BufferRange<'a>>) {
        let mut raw: SmallVec<_, 16> = Default::default();
        let mut offsets: SmallVec<_, 16> = Default::default();
        for buffer in buffers {
            raw.push(buffer.raw());
            offsets.push(buffer.offset());
        }
        assert!(!raw.is_empty());

        unsafe {
            self.dt().cmd_bind_vertex_buffers(
                self.raw(),
                0,
                raw.len() as _,
                raw.as_ptr(),
                offsets.as_ptr(),
            );
        }
    }

    fn pre_draw(&mut self) {
        self.ensure_recording();
        // TODO: Check vertex buffer bounds
        // TODO: Check bound descriptor sets
        assert!(self.gfx_pipe.is_some());
    }

    #[inline]
    pub unsafe fn draw(&mut self, vertex_count: u32, instance_count: u32) {
        self.draw_offset(vertex_count, instance_count, 0, 0);
    }

    pub unsafe fn draw_offset(
        &mut self,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) {
        self.pre_draw();
        self.dt().cmd_draw(
            self.raw(),
            vertex_count,
            instance_count,
            first_vertex,
            first_instance,
        );
    }

    #[inline]
    pub unsafe fn draw_indexed(&mut self, vertex_count: u32, instance_count: u32) {
        self.draw_indexed_offset(vertex_count, instance_count, 0, 0, 0);
    }

    pub unsafe fn draw_indexed_offset(
        &mut self,
        vertex_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
    ) {
        trace!(
            concat!(
                "SubpassCmds::draw_indexed_offset(vertex_count: {}, ",
                "instance_count: {}, first_index: {}, vertex_offset: {}, ",
                "first_instance: {})",
            ),
            vertex_count,
            instance_count,
            first_index,
            vertex_offset,
            first_instance,
        );
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

    fn check_state(&self) {
        if let Some(render_pass) = self.render_pass().as_ref() {
            let subpass_count = render_pass.subpasses().len();
            assert!((self.cur_subpass as usize) < subpass_count);
        }
        if self.level() == CmdBufferLevel::Primary {
            assert_eq!(self.cur_contents, SubpassContents::Inline);
        }
    }

    pub fn begin_render_pass(
        &mut self,
        framebuffer: Arc<Framebuffer>,
        clear_values: &[vk::ClearValue],
        contents: SubpassContents,
    ) {
        if !self.is_recording() {
            self.begin();
        }

        assert_eq!(self.level(), CmdBufferLevel::Primary);
        self.cur_subpass = 0;
        self.cur_contents = contents;
        self.check_state();

        if !self.is_recording() {
            self.begin();
        }
        assert!(framebuffer.is_swapchain_valid());

        // TODO: Clear color
        let begin_info = vk::RenderPassBeginInfo {
            render_pass: framebuffer.render_pass().inner(),
            framebuffer: framebuffer.inner(),
            render_area: framebuffer.render_area(),
            clear_value_count: clear_values.len() as _,
            p_clear_values: clear_values.as_ptr(),
            ..Default::default()
        };

        self.framebuffer = Some(framebuffer);
        self.reset_dynamic_state();
        unsafe {
            self.dt()
                .cmd_begin_render_pass(self.raw(), &begin_info, contents.into());
        }
    }

    pub fn next_subpass(&mut self, contents: SubpassContents) {
        assert_eq!(self.level(), CmdBufferLevel::Primary);
        self.ensure_recording();
        self.cur_subpass += 1;
        self.cur_contents = contents;
        self.check_state();
        unsafe {
            self.dt().cmd_next_subpass(self.raw(), contents.into());
        }
    }

    pub unsafe fn execute_cmds(&mut self, cmds: &[vk::CommandBuffer]) {
        assert_eq!(self.level(), CmdBufferLevel::Primary);
        assert_eq!(self.cur_contents, SubpassContents::Secondary);
        self.dt()
            .cmd_execute_commands(self.raw(), cmds.len() as _, cmds.as_ptr());
    }

    pub fn end_render_pass(&mut self) {
        unsafe {
            self.dt().cmd_end_render_pass(self.raw());
            self.framebuffer = None;
            self.gfx_pipe = None;
        }
    }

    pub unsafe fn pipeline_barrier(
        &mut self,
        src_stage_mask: vk::PipelineStageFlags,
        dst_stage_mask: vk::PipelineStageFlags,
        dependency_flags: vk::DependencyFlags,
        global_barriers: &[vk::MemoryBarrier],
        buffer_barriers: &[vk::BufferMemoryBarrier],
        image_barriers: &[vk::ImageMemoryBarrier],
    ) {
        trace!(
            concat!(
                "XferCmds::pipeline_barrier(",
                "src_stage_mask: {:?}, ",
                "dst_stage_mask: {:?}, ",
                "dependency_flags: {:?}, ",
                "global_barriers: {:?}, ",
                "buffer_barriers: {:?}, ",
                "image_barriers: {:?})",
            ),
            src_stage_mask,
            dst_stage_mask,
            dependency_flags,
            global_barriers,
            buffer_barriers,
            image_barriers
        );
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
    pub unsafe fn copy_buffer(
        &mut self,
        src: &Arc<DeviceBuffer>,
        dst: &Arc<DeviceBuffer>,
        regions: &[vk::BufferCopy],
    ) {
        self.ensure_recording();
        // This check is good for catching unnecessary copies on UMA.
        // However, there are use cases that may need to be allowed.
        assert!(
            !Arc::ptr_eq(src, dst),
            "copy to same buffer (likely unintended)"
        );
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

    pub unsafe fn copy_buffer_to_image(
        &mut self,
        src: &DeviceBuffer,
        dst: &Arc<Image>,
        layout: vk::ImageLayout,
        regions: &[vk::BufferImageCopy],
    ) {
        self.ensure_recording();
        trace!(
            concat!(
                "XferCmds::copy_buffer_to_image(src: {:?}, dst: {:?}, ",
                "layout: {:?}, regions: {:?})",
            ),
            fmt_named(&*src),
            fmt_named(&**dst),
            layout,
            regions
        );
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
}

#[cfg(debug_assertions)]
fn validate_buffer_image_copy(
    src: &DeviceBuffer,
    dst: &Image,
    layout: vk::ImageLayout,
    regions: &[vk::BufferImageCopy],
) {
    use math::Ivector3;

    assert!([
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        vk::ImageLayout::GENERAL,
        vk::ImageLayout::SHARED_PRESENT_KHR,
    ]
    .contains(&layout));
    for region in regions.iter() {
        let (x, y, z) = region.image_offset.into();
        let off = Ivector3::new(x, y, z);
        let ext = Extent3D::from(region.image_extent);
        assert!(dst.extent().contains_extent(off, ext));

        let texel_size = dst.format().size();
        let row_length = if region.buffer_row_length == 0 {
            region.buffer_row_length
        } else {
            region.image_extent.width
        } as usize;
        let image_height = if region.buffer_image_height == 0 {
            region.buffer_image_height
        } else {
            region.image_extent.height
        } as usize;
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
    use crate::testing::*;
    use crate::*;
    use std::sync::Arc;

    unsafe fn test_common(
        vars: &TestVars,
    ) -> (
        TestResources,
        PipelineCache,
        TrivialRenderer,
        TrivialPass,
        Vec<Arc<Framebuffer>>,
        Box<CmdPool>,
    ) {
        let device = vars.device();
        let resources = TestResources::new(device);
        let pipelines = PipelineCache::new(device);
        let trivial = TrivialRenderer::new(&resources);
        let pass = TrivialPass::new(device);
        let framebuffers = pass.create_framebuffers(vars.swapchain());
        let pool = Box::new(CmdPool::new(
            vars.gfx_queue().family(),
            vk::CommandPoolCreateFlags::TRANSIENT_BIT,
        ));
        (resources, pipelines, trivial, pass, framebuffers, pool)
    }

    #[test]
    fn record_subpass() {
        unsafe {
            let vars = TestVars::new();
            let (_res, pipelines, trivial, _pass, framebuffers, pool) = test_common(&vars);
            let framebuffer = Arc::clone(&framebuffers[0]);
            let mut cmds = CmdBuffer::new_subpass(pool, framebuffer, 0);
            trivial.render(&pipelines, &mut cmds);
            let (_, _) = cmds.end();
        }
    }

    #[test]
    fn record_render_pass() {
        unsafe {
            let vars = TestVars::new();
            let (_res, pipelines, trivial, _, framebuffers, pool) = test_common(&vars);
            let mut cmds = CmdBuffer::new(pool);
            let framebuffer = Arc::clone(&framebuffers[0]);
            cmds.begin_render_pass(framebuffer, &[], SubpassContents::Inline);
            trivial.render(&pipelines, &mut cmds);
            let (_, _) = cmds.end();
        }
    }

    #[test]
    #[should_panic]
    fn subpass_out_of_bounds() {
        unsafe {
            let vars = TestVars::new();
            let (_res, _, _, _, framebuffers, pool) = test_common(&vars);
            let mut cmds = CmdBuffer::new(pool);
            let framebuffer = Arc::clone(&framebuffers[0]);
            cmds.begin_render_pass(framebuffer, &[], SubpassContents::Inline);
            cmds.next_subpass(SubpassContents::Inline);
        }
    }

    #[test]
    #[should_panic]
    fn exec_in_inline_subpass() {
        unsafe {
            let vars = TestVars::new();
            let (_res, _, _, _, framebuffers, pool) = test_common(&vars);
            let framebuffer = Arc::clone(&framebuffers[0]);
            let mut cmds = CmdBuffer::new_subpass(pool, framebuffer, 0);
            cmds.execute_cmds(&[vk::null()]);
        }
    }

    fn copy_common(vars: &testing::TestVars) -> (TestResources, CmdBuffer) {
        let resources = TestResources::new(vars.device());
        let pool = Box::new(CmdPool::new(
            vars.gfx_queue().family(),
            vk::CommandPoolCreateFlags::TRANSIENT_BIT,
        ));
        let cmds = CmdBuffer::new(pool);
        (resources, cmds)
    }

    #[test]
    fn copy_buffer() {
        unsafe {
            let vars = TestVars::new();
            let (resources, mut cmds) = copy_common(&vars);
            let src = resources.buffer_heap.alloc(
                BufferBinding::Storage,
                Lifetime::Frame,
                MemoryMapping::Mapped,
                1024,
            );
            let dst = resources.buffer_heap.alloc(
                BufferBinding::Vertex,
                Lifetime::Frame,
                MemoryMapping::DeviceLocal,
                1024,
            );
            cmds.begin();
            cmds.copy_buffer(
                src.buffer(),
                dst.buffer(),
                &[
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
                ],
            );
            cmds.end();
        }
    }

    #[test]
    #[should_panic]
    fn copy_intra_buffer() {
        unsafe {
            let vars = TestVars::new();
            let (resources, mut cmds) = copy_common(&vars);
            let buf = resources.buffer_heap.alloc(
                BufferBinding::Storage,
                Lifetime::Frame,
                MemoryMapping::Mapped,
                1024,
            );
            cmds.begin();
            cmds.copy_buffer(
                buf.buffer(),
                buf.buffer(),
                &[
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
                ],
            );
            cmds.end();
        }
    }

    #[test]
    fn copy_image() {
        unsafe {
            let vars = TestVars::new();
            let (resources, mut cmds) = copy_common(&vars);
            let format = Format::RGBA8;
            let src = resources.buffer_heap.alloc(
                BufferBinding::Storage,
                Lifetime::Frame,
                MemoryMapping::Mapped,
                (64 * 64 * format.size()) as _,
            );
            let dst = Arc::new(Image::with(
                &resources.image_heap,
                ImageFlags::NO_SAMPLE,
                ImageType::Dim2,
                format,
                SampleCount::One,
                Extent3D::new(64, 64, 1),
                1,
                1,
            ));
            cmds.begin();
            cmds.copy_buffer_to_image(
                src.buffer(),
                &dst,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[vk::BufferImageCopy {
                    image_subresource: dst.all_layers_for_mip_level(0).to_mip_layers(0),
                    image_extent: dst.extent().into(),
                    ..Default::default()
                }],
            );
            cmds.end();
        }
    }
}
