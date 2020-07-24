use std::sync::Arc;

use crate::*;

const VERTEX_COUNT: u32 = 3;

#[derive(Debug)]
crate struct TrivialRenderer {
    globals: Arc<Globals>,
    set_layouts: [Arc<SetLayout>; 2],
    vert_shader: Arc<ShaderSpec>,
    frag_shader: Arc<ShaderSpec>,
    descs: [DescriptorSet; 2],
}

/// Render pass with a single subpass and single backbuffer attachment.
#[derive(Debug)]
crate struct TrivialPass {
    crate pass: Arc<RenderPass>,
    crate subpass: Subpass,
}

impl TrivialRenderer {
    crate const fn vertex_count() -> u32 {
        VERTEX_COUNT
    }

    crate fn new(state: &SystemState, globals: Arc<Globals>) -> Self {
        let device = Arc::clone(&state.device);

        let bindings = set_layout_bindings![
            (0, UNIFORM_BUFFER),
            (1, STORAGE_BUFFER),
        ];
        let layout0 = unsafe {
            Arc::new(SetLayout::from_bindings(Arc::clone(&device), &bindings))
        };

        let bindings = set_layout_bindings![
            (0, COMBINED_IMAGE_SAMPLER),
            (1, STORAGE_IMAGE),
            (2, SAMPLED_IMAGE),
        ];
        let layout1 = unsafe {
            Arc::new(SetLayout::from_bindings(Arc::clone(&device), &bindings))
        };

        let shaders = &globals.shaders;
        let vert_shader = Arc::new(Arc::clone(&shaders.trivial_vert).into());
        let frag_shader = Arc::new(Arc::clone(&shaders.trivial_frag).into());

        let descs = &state.descriptors;
        let mut descs = [
            descs.alloc(Lifetime::Static, &layout0),
            descs.alloc(Lifetime::Static, &layout1),
        ];
        for desc in descs.iter_mut() {
            unsafe { globals.write_empty_descriptors(desc); }
        }

        TrivialRenderer {
            globals,
            set_layouts: [layout0, layout1],
            vert_shader,
            frag_shader,
            descs,
        }
    }

    crate fn descriptor_layouts(&self) -> &[Arc<SetLayout>] {
        &self.set_layouts[..]
    }

    crate fn descriptors(&self) -> &[DescriptorSet] {
        &self.descs[..]
    }

    crate fn init_pipe_desc(&self, desc: &mut GraphicsPipelineDesc) {
        desc.layout.set_layouts = self.set_layouts.to_vec();
        desc.stages.insert(ShaderStage::Vertex, Arc::clone(&self.vert_shader));
        desc.stages.insert(ShaderStage::Fragment, Arc::clone(&self.frag_shader));
    }

    crate fn render(&self, state: &SystemState, cmds: &mut SubpassCmds) {
        let mut desc = GraphicsPipelineDesc::new(cmds.subpass().clone());
        self.init_pipe_desc(&mut desc);

        let pipe = unsafe { state.pipelines.get_or_create_gfx(&desc) };
        cmds.bind_gfx_pipe(&pipe);

        cmds.bind_gfx_descs(0, &self.descs[0]);
        cmds.bind_gfx_descs(1, &self.descs[1]);

        unsafe { cmds.draw(Self::vertex_count(), 1); }
    }
}

impl TrivialPass {
    crate fn new(device: Arc<Device>) -> Self {
        unsafe { create_trivial_pass(device) }
    }

    crate fn create_framebuffers(&self, swapchain: &Swapchain) ->
        Vec<Arc<Framebuffer>>
    {
        unsafe {
            swapchain.create_views().into_iter()
                .map(|view| Arc::new(Framebuffer::new(
                    Arc::clone(&self.pass),
                    vec![view.into()],
                )))
                .collect()
        }
    }
}

unsafe fn create_trivial_pass(device: Arc<Device>) -> TrivialPass {
    use vk::ImageLayout as Layout;
    let pass = RenderPass::new(
        device,
        vec![
            AttachmentDescription {
                name: Attachment::Backbuffer,
                format: Format::BGRA8_SRGB,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                ..Default::default()
            },
        ],
        vec![
            SubpassDesc {
                layouts: vec![Layout::COLOR_ATTACHMENT_OPTIMAL],
                color_attchs: vec![0],
                ..Default::default()
            },
        ],
        vec![],
    );

    let mut subpasses = pass.subpasses();
    TrivialPass {
        pass: Arc::clone(&pass),
        subpass: subpasses.next().unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let state = SystemState::new(Arc::clone(vars.device()));
        let heap = ImageHeap::new(Arc::clone(vars.device()));
        let globals = Arc::new(Globals::new(&state, &heap));
        let _ = TrivialRenderer::new(&state, globals);
    }

    unit::declare_tests![smoke_test];
}

unit::collect_tests![tests];
