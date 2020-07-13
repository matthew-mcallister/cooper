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

    unsafe fn create_mesh(vars: crate::testing::TestVars) {
        let state = SystemState::new(Arc::clone(vars.device()));

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
            state: &state,
            lifetime: Lifetime::Frame,
            mesh: Default::default(),
        };
        builder.index(IndexType::U16, idxs.as_bytes())
            .attr(VertexAttr::Position, Format::RGB32F, pos.as_bytes())
            .attr(VertexAttr::Normal, Format::RGB32F, normal.as_bytes());
        let _ = builder.build();
    }

    unit::declare_tests![
        create_mesh,
    ];
}

unit::collect_tests![tests];
