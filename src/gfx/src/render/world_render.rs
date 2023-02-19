use std::sync::Arc;

use device::*;
use more_asserts::assert_lt;
use smallvec::smallvec;

use crate::*;

#[derive(Debug)]
pub(crate) struct BasicPass {
    pub(crate) pass: Arc<RenderPass>,
    pub(crate) subpass: Subpass,
}

impl BasicPass {
    pub(crate) fn new(device: Arc<Device>) -> Self {
        let pass = unsafe { create_basic_pass(device) };
        let mut subpasses = pass.subpasses();
        BasicPass {
            pass: Arc::clone(&pass),
            subpass: subpasses.next().unwrap(),
        }
    }

    pub(crate) fn create_framebuffers(
        &self,
        heap: &ImageHeap,
        swapchain: &Swapchain,
    ) -> Vec<Arc<Framebuffer>> {
        unsafe {
            swapchain
                .create_views()
                .into_iter()
                .map(|view| {
                    let depth_view =
                        create_render_target(heap, &self.pass, 1, swapchain.extent(), false);
                    Arc::new(Framebuffer::new(
                        Arc::clone(&self.pass),
                        vec![view.into(), depth_view.into()],
                    ))
                })
                .collect()
        }
    }
}

unsafe fn create_basic_pass(device: Arc<Device>) -> Arc<RenderPass> {
    use vk::ImageLayout as Il;
    RenderPass::new(
        device,
        vec![
            AttachmentDescription {
                name: Attachment::Backbuffer,
                format: Format::BGRA8_SRGB,
                load_op: vk::AttachmentLoadOp::CLEAR,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                ..Default::default()
            },
            AttachmentDescription {
                name: Attachment::DepthStencil,
                format: Format::D32F,
                load_op: vk::AttachmentLoadOp::CLEAR,
                // TODO: Maybe initial_layout should equal final_layout.
                // But that would require a manual layout transition
                // before the first frame.
                final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                ..Default::default()
            },
        ],
        vec![SubpassDesc {
            layouts: vec![
                Il::COLOR_ATTACHMENT_OPTIMAL,
                Il::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            ],
            color_attchs: vec![0],
            depth_stencil_attch: Some(1),
            ..Default::default()
        }],
        vec![],
    )
}

/// Top-level renderer.
// NB: Try to ensure this this type persists no state between frames.
#[derive(Debug)]
pub(crate) struct WorldRenderer {
    globals: Arc<Globals>,
    scheduler: RenderScheduler,
    basic_pass: BasicPass,
    framebuffers: Vec<Arc<Framebuffer>>,
    clear_values: [vk::ClearValue; 2],
}

impl WorldRenderer {
    pub(crate) fn new(
        state: &SystemState,
        heap: &ImageHeap,
        globals: Arc<Globals>,
        swapchain: &Swapchain,
        gfx_queue: Arc<Queue>,
    ) -> Self {
        let scheduler = RenderScheduler::new(gfx_queue);
        let basic_pass = BasicPass::new(Arc::clone(&state.device));
        let framebuffers = basic_pass.create_framebuffers(&heap, &swapchain);
        let clear_values = [clear_color([0.0; 4]), clear_depth(0.0)];
        Self {
            globals,
            scheduler,
            basic_pass,
            framebuffers,
            clear_values,
        }
    }

    /// Used when recreating the swapchain
    #[allow(dead_code)]
    pub(crate) fn into_inner(self) -> RenderScheduler {
        self.scheduler
    }

    pub(crate) fn base_desc(&self, base_layout: &Arc<DescriptorSetLayout>) -> GraphicsPipelineDesc {
        let subpass = self.basic_pass.subpass.clone();
        let mut desc = GraphicsPipelineDesc::new(subpass);
        desc.layout.set_layouts = smallvec![Arc::clone(base_layout); 2];
        // FIXME: Supposed to be part of material
        desc.cull_mode = CullMode::Back;
        desc.depth_test = true;
        desc.depth_write = true;
        desc.depth_cmp_op = vk::CompareOp::GREATER;
        desc
    }

    pub(crate) fn create_pipelines(
        &self,
        base_layout: &Arc<DescriptorSetLayout>,
        state: &mut SystemState,
        materials: &mut MaterialStateTable,
    ) {
        let mut base = self.base_desc(base_layout);
        unsafe {
            materials.create_pipelines(state, &mut base);
        }
    }

    fn objects_pass(
        &mut self,
        state: &Arc<Box<SystemState>>,
        resources: &ResourceSystem,
        materials: &MaterialStateTable,
        descriptors: DescriptorSet,
        pass: &mut RenderPassNode,
        objects: Vec<RenderObject>,
    ) {
        // TODO: It should be possible to get this code working when
        // `objects` is empty
        if objects.is_empty() {
            return;
        }

        let items: Vec<_> =
            lower_objects(&state, resources, &materials, objects.into_iter()).collect();

        let mut inst = InstanceRenderer::new(&state, &self.globals);
        pass.add_task(
            0,
            Box::new(move |cmds| {
                inst.render(&descriptors, &items, cmds);
            }),
        );
    }

    pub(crate) fn run(
        &mut self,
        state: Arc<Box<SystemState>>,
        resources: &ResourceSystem,
        materials: &MaterialStateTable,
        world: RenderWorldData,
        frame_num: u64,
        swapchain_image: u32,
        acquire_sem: &mut BinarySemaphore,
        present_sem: &mut BinarySemaphore,
        render_sem: &mut TimelineSemaphore,
    ) {
        unsafe {
            self.scheduler.clear();
        }

        let framebuffer = Arc::clone(&self.framebuffers[swapchain_image as usize]);
        let clear_values = self.clear_values.to_vec();
        let mut pass = RenderPassNode::with_clear(framebuffer, clear_values);

        // Check that indices used in shaders are in bounds
        for object in world.objects.iter() {
            match object {
                RenderObject::MeshInstance(obj) => {
                    assert_lt!(obj.xform_index as usize, world.uniforms.xforms.len(),)
                }
            }
        }
        let descriptors = world.uniforms.create_descriptor_set(&state);
        let objects = world.objects;
        self.objects_pass(
            &state,
            resources,
            materials,
            descriptors,
            &mut pass,
            objects,
        );

        self.scheduler.schedule_pass(
            pass,
            &[WaitInfo {
                semaphore: acquire_sem.inner_mut(),
                value: 0,
                stages: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT_BIT,
            }],
            &[
                SignalInfo {
                    semaphore: present_sem.inner_mut(),
                    value: 0,
                },
                SignalInfo {
                    semaphore: render_sem.inner_mut(),
                    value: frame_num,
                },
            ],
        );
    }
}
