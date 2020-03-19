use std::sync::Arc;

use enum_map::{Enum, EnumMap};

use crate::*;

#[derive(Clone, Copy, Debug, Enum, Eq, Hash, PartialEq)]
crate enum DebugDisplay {
    Depth = 0,
    Normal = 1,
}

#[derive(Debug)]
crate struct DebugMesh {
    crate mesh: Arc<RenderMesh>,
    crate display: DebugDisplay,
    crate mv: [[f32; 4]; 4],
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C, align(16))]
crate struct DebugInstance {
    crate mv: [[f32; 4]; 4],
}

// Minimal mesh rendering for visualization and debugging.
#[derive(Debug)]
crate struct DebugRenderer {
    globals: Arc<Globals>,
    pipe_layout: Arc<PipelineLayout>,
    vert_shader: Arc<ShaderSpec>,
    frag_shaders: EnumMap<DebugDisplay, Arc<ShaderSpec>>,
    desc_set: DescriptorSet,
}

impl DebugRenderer {
    crate fn new(state: &SystemState, globals: Arc<Globals>) -> Self {
        let device = Arc::clone(&state.device);

        let vert_shader =
            Arc::new(Arc::clone(&globals.shaders.static_vert).into());
        // TODO: Maybe using spec constants is overkill
        let frag_shaders = (|display| {
            let shader = Arc::clone(&globals.shaders.debug_frag);
            let mut spec = ShaderSpec::new(shader);
            spec.set(ShaderConst::DebugDisplay as _, &(display as u32));
            Arc::new(spec)
        }).into();

        let set_layout = Self::create_set_layout(Arc::clone(&device));
        let desc_set = state.descriptors.lock().alloc(&set_layout);
        let pipe_layout = Arc::new(PipelineLayout::new(
            Arc::clone(&device),
            vec![set_layout],
        ));

        Self {
            globals,
            pipe_layout,
            vert_shader,
            frag_shaders,
            desc_set,
        }
    }

    crate fn create_set_layout(device: Arc<Device>) -> Arc<DescriptorSetLayout>
    {
        let bindings = [
            vk::DescriptorSetLayoutBinding {
                binding: 0,
                descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::VERTEX_BIT
                    | vk::ShaderStageFlags::FRAGMENT_BIT,
                ..Default::default()
            },
            vk::DescriptorSetLayoutBinding {
                binding: 1,
                descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::VERTEX_BIT,
                ..Default::default()
            },
        ];
        unsafe {
            Arc::new(DescriptorSetLayout::from_bindings(device, &bindings))
        }
    }

    unsafe fn update_descriptors(
        &mut self,
        view: &SceneView,
        meshes: &[DebugMesh],
    ) {
        self.desc_set.write_buffer(0, view.view_uniforms());
        let instances = view.state.buffers.box_iter(
            BufferBinding::Storage,
            Lifetime::Frame,
            meshes.iter().map(|mesh| DebugInstance { mv: mesh.mv }),
        );
        self.desc_set.write_buffer(1, instances.range());
    }

    crate fn render(
        &mut self,
        view: &SceneView,
        meshes: Vec<DebugMesh>,
        cmds: &mut SubpassCmds,
    ) {
        unsafe { self.render_unsafe(view, meshes, cmds); }
    }

    // TODO: Sort meshes by pipeline, or at least display type
    crate unsafe fn render_unsafe(
        &mut self,
        view: &SceneView,
        meshes: Vec<DebugMesh>,
        cmds: &mut SubpassCmds,
    ) {
        let state = view.state();
        self.update_descriptors(view, &meshes);

        let mut desc = GraphicsPipelineDesc::new(
            cmds.subpass().clone(),
            Arc::clone(&self.pipe_layout),
        );
        desc.stages[ShaderStage::Vertex] = Some(Arc::clone(&self.vert_shader));

        let displays = [
            DebugDisplay::Depth,
            DebugDisplay::Normal,
        ];
        for &display in displays.iter() {
            desc.stages[ShaderStage::Fragment] =
                Some(Arc::clone(&self.frag_shaders[display]));
            for (i, mesh) in meshes.iter()
                .filter(|mesh| mesh.display == display).enumerate()
            {
                let mesh = &mesh.mesh;
                let inst = i as u32;

                desc.vertex_layout = VertexInputLayout::new(
                    &mesh.vertex_layout(),
                    self.vert_shader.shader(),
                );
                desc.cull_mode = view.cull_mode;
                let pipeline = state.gfx_pipes.get_or_create(&desc);
                cmds.bind_gfx_pipe(&pipeline);

                cmds.bind_gfx_descs(0, &self.desc_set);

                let vert_count = 3 * mesh.tri_count;
                cmds.bind_vertex_buffers(&mesh.data());
                if let Some(ref index) = &mesh.index {
                    cmds.bind_index_buffer(index.alloc.range(), index.ty);
                    cmds.draw_indexed_offset(vert_count, 1, 0, 0, inst);
                } else {
                    cmds.draw_offset(vert_count, 1, 0, inst);
                }
            }
        }
    }
}
