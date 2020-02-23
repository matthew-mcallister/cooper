use enum_map::EnumMap;

use crate::*;

#[derive(Debug)]
crate struct RenderMesh {
    crate tri_count: u32,
    crate index: Option<IndexBuffer>,
    crate bindings: EnumMap<VertexAttrName, Option<AttrBuffer>>,
}

#[derive(Debug)]
crate struct AttrBuffer {
    crate alloc: BufferAlloc,
    crate format: Format,
}

#[derive(Debug)]
crate struct IndexBuffer {
    crate alloc: BufferAlloc,
    crate ty: IndexType,
}

impl IndexType {
    pub fn size(self) -> usize {
        match self {
            Self::U16 => 2,
            Self::U32 => 4,
        }
    }
}

impl RenderMesh {
    crate fn vertex_layout(&self) -> VertexLayout {
        let attrs = |name| Some(VertexAttr {
            format: self.bindings[name].as_ref()?.format,
        });
        VertexLayout {
            topology: PrimitiveTopology::TriangleList,
            packing: VertexPacking::Unpacked,
            attrs: attrs.into(),
        }
    }

    crate fn data(&self) -> VertexData<'_> {
        let bindings = |name| Some({
            self.bindings[name].as_ref()?.alloc.range()
        });
        VertexData::Unpacked(bindings.into())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use super::*;

    unsafe fn create_mesh(state: &SystemState) -> RenderMesh {
        let positions = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        ];
        let idxs = [
            0u16, 3, 1,
            0, 2, 3,
        ];
        let pos = state.buffers.box_slice(BufferBinding::Vertex, &positions);
        let idxs = state.buffers.box_slice(BufferBinding::Index, &idxs);
        RenderMesh {
            tri_count: 2,
            index: Some(IndexBuffer {
                alloc: idxs.into_inner(),
                ty: IndexType::U16,
            }),
            bindings: enum_map(std::iter::once((
                VertexAttrName::Position,
                Some(AttrBuffer {
                    alloc: pos.into_inner(),
                    format: Format::RGB32F,
                }),
            ))),
        }
    }

    unsafe fn bind_test(vars: testing::TestVars) {
        let device = Arc::clone(vars.device());
        let dev = || Arc::clone(&device);

        // TODO: Slim down test boilerplate
        let state = SystemState::new(dev());
        let globals = Arc::new(Globals::new(&state));

        let mesh = create_mesh(&state);

        let pass = TrivialPass::new(dev());
        let framebuffers = pass.create_framebuffers(&vars.swapchain);

        let pool = Box::new(CmdPool::new(
            vars.gfx_queue().family(),
            vk::CommandPoolCreateFlags::TRANSIENT_BIT,
        ));
        let mut cmds = RenderPassCmds::new(
            CmdBuffer::new(pool, CmdBufferLevel::Primary),
            Arc::clone(&framebuffers[0]),
            SubpassContents::Inline,
        ).enter_subpass();

        let pipe_layout = Arc::new(PipelineLayout::new(dev(), vec![]));
        let mut desc =
            GraphicsPipelineDesc::new(cmds.subpass().clone(), pipe_layout);

        let shaders = &globals.shaders;
        desc.stages[ShaderStage::Vertex] =
            Some(Arc::new(Arc::clone(&shaders.static_vert).into()));
        desc.stages[ShaderStage::Fragment] =
            Some(Arc::new(Arc::clone(&shaders.debug_depth_frag).into()));

        desc.vertex_layout = mesh.vertex_layout()
            .to_input_layout(desc.vertex_stage().unwrap().shader());

        let pipe = state.gfx_pipes.get_or_create(&desc);
        cmds.bind_gfx_pipe(&pipe);

        let idx = mesh.index.as_ref().unwrap();
        cmds.bind_index_buffer(idx.alloc.range(), idx.ty);

        cmds.bind_vertex_buffers(&mesh.data());
        cmds.draw_indexed(3 * mesh.tri_count, 1);

        let (_, _) = cmds.exit_subpass().end().end();
    }

    unit::declare_tests![
        bind_test,
    ];
}

unit::collect_tests![tests];
