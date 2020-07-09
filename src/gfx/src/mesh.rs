use base::PartialEnumMap;
use itertools::Itertools;

use crate::*;

// TODO: Might need bbox property
#[derive(Debug, Default)]
pub struct RenderMesh {
    vertex_count: u32,
    index: Option<IndexBuffer>,
    bindings: PartialEnumMap<VertexAttr, AttrBuffer>,
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

/// Allows building a mesh without directly using the device interface.
#[derive(Debug)]
pub struct RenderMeshBuilder<'a> {
    state: &'a SystemState,
    lifetime: Lifetime,
    mesh: RenderMesh,
}

impl RenderMesh {
    /// The number of vertices in the mesh.
    crate fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    crate fn index(&self) -> Option<&IndexBuffer> {
        self.index.as_ref()
    }

    crate fn bindings(&self) -> &PartialEnumMap<VertexAttr, AttrBuffer> {
        &self.bindings
    }

    crate fn vertex_layout(&self) -> VertexLayout {
        VertexLayout {
            topology: PrimitiveTopology::TriangleList,
            packing: VertexPacking::Unpacked,
            attrs: self.bindings.iter()
                .map(|(name, binding)| {
                    (name, VertexLayoutAttr { format: binding.format })
                })
                .collect(),
        }
    }

    crate fn data(&self) -> VertexData<'_> {
        VertexData::Unpacked(
            self.bindings.iter()
                .map(|(name, binding)| (name, binding.alloc.range()))
                .collect()
        )
    }
}

impl<'a> RenderMeshBuilder<'a> {
    pub fn from_world(world: &'a RenderWorld) -> Self {
        Self {
            state: world.state(),
            lifetime: Lifetime::Static,
            mesh: Default::default(),
        }
    }

    pub fn from_loop(rloop: &'a RenderLoop) -> Self {
        Self {
            state: rloop.state(),
            lifetime: Lifetime::Static,
            mesh: Default::default(),
        }
    }

    pub fn lifetime(&mut self, lifetime: Lifetime) -> &mut Self {
        self.lifetime = lifetime;
        self
    }

    pub fn index(&mut self, ty: IndexType, data: &[u8]) -> &mut Self {
        assert_eq!(data.len() % ty.size(), 0);
        let binding = BufferBinding::Index;
        let lifetime = self.lifetime;
        let alloc = self.state.buffers.box_slice(binding, lifetime, data)
            .into_inner();
        self.mesh.index = Some(IndexBuffer { alloc, ty });
        self
    }

    pub fn attr(&mut self, attr: VertexAttr, format: Format, data: &[u8]) ->
        &mut Self
    {
        assert_eq!(data.len() % format.size(), 0);
        let binding = BufferBinding::Vertex;
        let lifetime = self.lifetime;
        let alloc = self.state.buffers.box_slice(binding, lifetime, data)
            .into_inner();
        self.mesh.bindings.insert(attr, AttrBuffer { alloc, format });
        self
    }

    fn set_vertex_count(&mut self) {
        // TODO: Why doesn't enum_map::Values implement Clone!
        let (min, max) = self.mesh.bindings.values()
            .map(|attr| attr.count())
            .minmax().into_option()
            .unwrap();
        assert_eq!(min, max);
        self.mesh.vertex_count = min;
    }

    pub unsafe fn build(mut self) -> RenderMesh {
        self.set_vertex_count();
        self.mesh
    }
}

impl AttrBuffer {
    /// The number of elements in the buffer.
    crate fn count(&self) -> u32 {
        (self.alloc.size() / self.format.size() as vk::DeviceSize) as _
    }
}

impl IndexBuffer {
    /// The number of elements in the buffer.
    crate fn count(&self) -> u32 {
        (self.alloc.size() / self.ty.size() as vk::DeviceSize) as _
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use prelude::*;
    use super::*;

    unsafe fn create_mesh(state: &SystemState) -> RenderMesh {
        let pos: &[[f32; 3]] = &[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        ];
        let normal: &[[f32; 3]] = &[[0.0, 0.0, -1.0]; 4];
        let idxs: &[u16] = &[
            0, 3, 1,
            0, 2, 3,
        ];
        let mut builder = RenderMeshBuilder {
            state,
            lifetime: Lifetime::Frame,
            mesh: Default::default(),
        };
        builder.index(IndexType::U16, idxs.as_bytes())
            .attr(VertexAttr::Position, Format::RGB32F, pos.as_bytes())
            .attr(VertexAttr::Normal, Format::RGB32F, normal.as_bytes());
        builder.build()
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
            &[],
            SubpassContents::Inline,
        ).enter_subpass();

        let layouts = vec![
            Arc::clone(&globals.scene_unifs_layout),
            Arc::clone(&globals.instance_buf_layout),
        ];
        let mut scene_unifs = state.descriptors.alloc(
            Lifetime::Frame,
            &layouts[0],
        );
        let mut inst_unifs = state.descriptors.alloc(
            Lifetime::Frame,
            &layouts[1],
        );
        globals.write_empty_descriptors(&mut scene_unifs);
        globals.write_empty_descriptors(&mut inst_unifs);

        let mut desc = GraphicsPipelineDesc::new(cmds.subpass().clone());

        let shaders = &globals.shaders;
        let vertex_stage = Arc::new(Arc::clone(&shaders.static_vert).into());
        desc.stages.insert(ShaderStage::Vertex, vertex_stage);
        let fragment_stage = Arc::new(Arc::clone(&shaders.simple_frag).into());
        desc.stages.insert(ShaderStage::Fragment, fragment_stage);

        desc.layout.set_layouts = layouts;
        desc.vertex_layout = mesh.vertex_layout()
            .to_input_layout(desc.vertex_stage().shader());

        let pipe = state.pipelines.get_or_create_gfx(&desc);
        cmds.bind_gfx_pipe(&pipe);

        cmds.bind_gfx_descs(0, &scene_unifs);
        cmds.bind_gfx_descs(1, &inst_unifs);

        let idx = mesh.index.as_ref().unwrap();
        cmds.bind_index_buffer(idx.alloc.range(), idx.ty);

        cmds.bind_vertex_buffers(&mesh.data());
        cmds.draw_indexed(mesh.vertex_count, 1);

        let (_, _) = cmds.exit_subpass().end().end();
    }

    unit::declare_tests![
        bind_test,
    ];
}

unit::collect_tests![tests];
