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
}

// Minimal mesh rendering for visualization and debugging.
#[derive(Debug)]
crate struct DebugRenderer {
    globals: Arc<Globals>,
    pipe_layout: Arc<PipelineLayout>,
    vert_shader: Arc<ShaderSpec>,
    frag_shaders: EnumMap<DebugDisplay, Arc<ShaderSpec>>,
}

impl DebugRenderer {
    crate fn new(state: &SystemState, globals: Arc<Globals>) -> Self {
        let device = Arc::clone(&state.device);
        let pipe_layout = Arc::new(PipelineLayout::new(device, vec![]));
        let vert_shader =
            Arc::new(Arc::clone(&globals.shaders.static_vert).into());
        let frag_shaders = (|display| {
            let shader = Arc::clone(&globals.shaders.debug_frag);
            let mut spec = ShaderSpec::new(shader);
            spec.set(ShaderConst::DebugDisplay as _, &(display as u32));
            Arc::new(spec)
        }).into();
        Self {
            globals,
            pipe_layout,
            vert_shader,
            frag_shaders,
        }
    }

    crate fn render(
        &mut self,
        state: &SystemState,
        meshes: Vec<DebugMesh>,
        cmds: &mut SubpassCmds,
    ) {
        unsafe { self.render_unsafe(state, meshes, cmds); }
    }

    // TODO: Sort meshes
    crate unsafe fn render_unsafe(
        &mut self,
        state: &SystemState,
        meshes: Vec<DebugMesh>,
        cmds: &mut SubpassCmds,
    ) {
        let displays = [
            DebugDisplay::Depth,
            DebugDisplay::Normal,
        ];

        let mut desc = GraphicsPipelineDesc::new(
            cmds.subpass().clone(),
            Arc::clone(&self.pipe_layout),
        );
        desc.stages[ShaderStage::Vertex] = Some(Arc::clone(&self.vert_shader));

        for &display in displays.iter() {
            desc.stages[ShaderStage::Fragment] =
                Some(Arc::clone(&self.frag_shaders[display]));
            for mesh in meshes.iter().filter(|mesh| mesh.display == display) {
                let mesh = &mesh.mesh;

                desc.vertex_layout = VertexInputLayout::new(
                    &mesh.vertex_layout(),
                    self.vert_shader.shader(),
                );
                let pipeline = state.gfx_pipes.get_or_create(&desc);
                cmds.bind_gfx_pipe(&pipeline);

                let vert_count = 3 * mesh.tri_count;
                cmds.bind_vertex_buffers(&mesh.data());
                if let Some(ref index) = &mesh.index {
                    cmds.bind_index_buffer(index.alloc.range(), index.ty);
                    cmds.draw_indexed(vert_count, 1);
                } else {
                    cmds.draw(vert_count, 1);
                }
            }
        }
    }
}
