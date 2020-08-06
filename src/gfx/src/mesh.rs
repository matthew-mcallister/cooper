use std::sync::Arc;

use base::PartialEnumMap;
use device::*;

use crate::*;

#[derive(Debug, Default)]
pub struct RenderMesh {
    vertex_count: u32,
    index: Option<IndexBuffer<BufferDef>>,
    bindings: PartialEnumMap<VertexAttr, AttrBuffer<BufferDef>>,
    // FIXME: This is a band-aid solution
    static_layout: VertexInputLayout,
}

#[derive(Debug)]
crate struct AttrBuffer<B> {
    crate buffer: Arc<B>,
    crate format: Format,
}

#[derive(Debug)]
crate struct IndexBuffer<B> {
    crate buffer: Arc<B>,
    crate ty: IndexType,
}

impl RenderMesh {
    /// The number of vertices in the mesh.
    pub fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    crate fn index(&self) -> Option<&IndexBuffer<BufferDef>> {
        self.index.as_ref()
    }

    crate fn bindings(&self) ->
        &PartialEnumMap<VertexAttr, AttrBuffer<BufferDef>>
    {
        &self.bindings
    }

    pub fn static_layout(&self) -> VertexInputLayout {
        self.static_layout.clone()
    }

    crate fn vertex_layout(&self) -> VertexStreamLayout {
        VertexStreamLayout {
            topology: PrimitiveTopology::TriangleList,
            attributes: self.bindings.iter()
                .map(|(name, binding)| {
                    (name, VertexStreamAttr { format: binding.format })
                })
                .collect(),
        }
    }
}

impl AttrBuffer<BufferDef> {
    fn count(&self) -> u32 {
        self.buffer.size as u32 / self.format.size() as u32
    }
}

impl IndexBuffer<BufferDef> {
    fn count(&self) -> u32 {
        self.buffer.size as u32 / self.ty.size() as u32
    }
}

#[derive(Debug)]
pub struct RenderMeshBuilder<'a> {
    rloop: &'a mut RenderLoop,
    lifetime: Lifetime,
    mesh: RenderMesh,
}

impl<'a> RenderMeshBuilder<'a> {
    pub fn from_world(world: &'a mut RenderWorld) -> Self {
        Self::from_loop(&mut world.rloop)
    }

    pub fn from_loop(rloop: &'a mut RenderLoop) -> Self {
        Self {
            rloop,
            lifetime: Lifetime::Static,
            mesh: Default::default(),
        }
    }

    pub fn lifetime(&mut self, lifetime: Lifetime) -> &mut Self {
        self.lifetime = lifetime;
        self
    }

    pub fn index(
        &mut self,
        ty: IndexType,
        src: Arc<Vec<u8>>,
        src_offset: usize,
        src_len: usize,
    ) -> &mut Self {
        assert_eq!(src_len % ty.size(), 0);
        let buffer = self.rloop.define_buffer(
            BufferBinding::Index,
            self.lifetime,
            MemoryMapping::DeviceLocal,
            src_len as _,
        );
        self.rloop.upload_buffer(&buffer, src, src_offset);
        self.mesh.index = Some(IndexBuffer { buffer, ty });
        self
    }

    pub fn attr(
        &mut self,
        attr: VertexAttr,
        format: Format,
        src: Arc<Vec<u8>>,
        src_offset: usize,
        src_len: usize,
    ) -> &mut Self {
        assert_eq!(src_len % format.size(), 0);
        let buffer = self.rloop.define_buffer(
            BufferBinding::Vertex,
            self.lifetime,
            MemoryMapping::DeviceLocal,
            src_len as _,
        );
        self.rloop.upload_buffer(&buffer, src, src_offset);
        self.mesh.bindings.insert(attr, AttrBuffer { buffer, format });
        self
    }

    fn set_vertex_count(&mut self) {
        if let Some(index) = &self.mesh.index {
            self.mesh.vertex_count = index.count();
        } else {
            let mut counts = self.mesh.bindings.values()
                .map(|attr| attr.count());
            let count = counts.next().unwrap();
            for other in counts { assert_eq!(other, count); }
            self.mesh.vertex_count = count;
        }
    }

    fn set_static_layout(&mut self) {
        let shader = &self.rloop.shaders().static_vert;
        self.mesh.static_layout = self.mesh.vertex_layout()
            .input_layout_for_shader(shader);
    }

    pub unsafe fn build(mut self) -> RenderMesh {
        self.set_vertex_count();
        self.set_static_layout();
        self.mesh
    }
}
