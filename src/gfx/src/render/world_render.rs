use std::sync::Arc;

use crate::*;

#[derive(Debug)]
crate struct BasicPass {
    crate pass: Arc<RenderPass>,
    crate subpass: Subpass,
}

impl BasicPass {
    crate fn new(device: Arc<Device>) -> Self {
        let pass = unsafe { create_basic_pass(device) };
        let mut subpasses = pass.subpasses();
        BasicPass {
            pass: Arc::clone(&pass),
            subpass: subpasses.next().unwrap(),
        }
    }

    crate fn create_framebuffers(
        &self,
        state: &SystemState,
        swapchain: &Swapchain,
    ) -> Vec<Arc<Framebuffer>> {
        unsafe {
            swapchain.create_views().into_iter()
                .map(|view| {
                    let depth_view = create_render_target(
                        state,
                        &self.pass,
                        1,
                        swapchain.extent(),
                        false,
                    );
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
        vec![
            SubpassDesc {
                layouts: vec![
                    Il::COLOR_ATTACHMENT_OPTIMAL,
                    Il::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                ],
                color_attchs: vec![0],
                depth_stencil_attch: Some(1),
                ..Default::default()
            },
        ],
        vec![],
    )
}

/// Top-level renderer.
#[derive(Debug)]
crate struct WorldRenderer {
    globals: Arc<Globals>,
    scheduler: RenderScheduler,
    basic_pass: BasicPass,
    framebuffers: Vec<Arc<Framebuffer>>,
    clear_values: [vk::ClearValue; 2],
    materials: MaterialSystem,
}

impl WorldRenderer {
    crate fn new(
        state: &SystemState,
        globals: Arc<Globals>,
        swapchain: &Swapchain,
        gfx_queue: Arc<Queue>,
    ) -> Self {
        let scheduler = RenderScheduler::new(gfx_queue);
        let basic_pass = BasicPass::new(Arc::clone(&state.device));
        let framebuffers = basic_pass.create_framebuffers(state, &swapchain);
        let clear_values = [clear_color([0.0; 4]), clear_depth(0.0)];
        let materials = MaterialSystem::new(state, &globals);
        Self {
            globals,
            scheduler,
            basic_pass,
            framebuffers,
            clear_values,
            materials,
        }
    }

    /// Used when recreating the swapchain
    crate fn into_inner(self) -> RenderScheduler {
        self.scheduler
    }

    crate fn materials(&self) -> &MaterialSystem {
        &self.materials
    }

    fn objects_pass(
        &mut self,
        state: &Arc<Box<SystemState>>,
        resources: &ResourceSystem,
        mut descriptors: SceneDescriptors,
        view: SceneViewState,
        pass: &mut RenderPassNode,
        objects: Vec<RenderObject>,
    ) {
        // TODO: It should be possible to get this code working when
        // `objects` is empty
        if objects.is_empty() { return; }

        let items: Vec<_> = lower_objects(
            &state, &view.uniforms, resources, &mut self.materials,
            &mut descriptors, objects.into_iter(),
        ).collect();

        let mut inst = InstanceRenderer::new(&state, &self.globals);
        pass.add_task(0, Box::new(move |cmds| {
            inst.render(&view, &descriptors, &items, cmds);
        }));
    }

    crate fn run(
        &mut self,
        state: Arc<Box<SystemState>>,
        resources: &ResourceSystem,
        world: RenderWorldData,
        frame_num: u64,
        swapchain_image: u32,
        acquire_sem: &mut BinarySemaphore,
        present_sem: &mut BinarySemaphore,
        render_sem: &mut TimelineSemaphore,
    ) {
        unsafe { self.scheduler.clear(); }

        let framebuffer =
            Arc::clone(&self.framebuffers[swapchain_image as usize]);
        let clear_values = self.clear_values.to_vec();
        let mut pass = RenderPassNode::with_clear(framebuffer, clear_values);

        let mut descriptors = SceneDescriptors::new(&state, &self.globals);
        let view = SceneViewState::new(&world.view);
        view.write_descriptor(&state, &mut descriptors);

        let objects = world.objects; // TODO: reuse this memory
        self.objects_pass(
            &state, resources, descriptors, view, &mut pass, objects);

        self.scheduler.schedule_pass(
            pass,
            &[acquire_sem.inner()],
            &[0],
            &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT_BIT],
            &[present_sem.inner(), render_sem.inner()],
            &[0, frame_num],
            None,
        );
    }
}
