use std::sync::Arc;

use device::{BufferAlloc, DescriptorSet, GraphicsPipeline, VertexInputLayout};
use log::debug;
use more_asserts::assert_gt;
use prelude::*;

use crate::{ResourceUnavailable, SystemState};
use crate::material::MaterialStateTable;
use crate::mesh::{AttrBuffer, IndexBuffer, RenderMesh};
use crate::resource::ResourceSystem;
use crate::util::SmallVec;
use super::*;

#[derive(Debug)]
crate struct RenderItem {
    crate mesh: MeshData,
    crate pipeline: Arc<GraphicsPipeline>,
    crate descriptors: Arc<DescriptorSet>,
    crate instance: u32,
}

#[derive(Debug)]
crate struct MeshData {
    crate vertex_count: u32,
    crate index: Option<IndexBuffer<BufferAlloc>>,
    crate attrs: SmallVec<AttrBuffer<BufferAlloc>, 6>,
}

#[allow(dead_code)]
impl MeshData {
    crate fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    crate fn index(&self) -> Option<&IndexBuffer<BufferAlloc>> {
        self.index.as_ref()
    }

    crate fn attrs(&self) -> &[AttrBuffer<BufferAlloc>] {
        &self.attrs
    }
}

#[derive(Debug)]
struct LowerCtx<'ctx> {
    state: &'ctx SystemState,
    resources: &'ctx ResourceSystem,
    materials: &'ctx MaterialStateTable,
}

trait Lower {
    fn lower<'ctx>(self, instance: u32, ctx: &mut LowerCtx<'ctx>) ->
        Result<RenderItem, ResourceUnavailable>;
}

impl<'ctx> LowerCtx<'ctx> {
    fn new(
        state: &'ctx SystemState,
        resources: &'ctx ResourceSystem,
        materials: &'ctx MaterialStateTable,
        object_count: usize,
    ) -> Self {
        assert_gt!(object_count, 0);
        Self { state, resources, materials }
    }
}

crate fn lower_objects<'a>(
    state: &'a SystemState,
    resources: &'a ResourceSystem,
    materials: &'a MaterialStateTable,
    objects: impl ExactSizeIterator<Item = RenderObject> + 'a,
) -> impl Iterator<Item = RenderItem> + 'a {
    let mut ctx = LowerCtx::new(
        state, resources, materials, objects.len());
    // TODO: Allow overriding the action to take when lowering fails
    objects.into_iter().enumerate().filter_map(move |(i, obj)| {
        obj.lower(i as _, &mut ctx)
            .on_err(|e| debug!("couldn't render object {}: {}", i, e))
            .ok()
    })
}

impl Lower for RenderObject {
    fn lower(self, instance: u32, ctx: &mut LowerCtx<'_>) ->
        Result<RenderItem, ResourceUnavailable>
    {
        match self {
            RenderObject::MeshInstance(mesh) => mesh.lower(instance, ctx),
        }
    }
}

impl Lower for MeshInstance {
    fn lower(self, _instance: u32, ctx: &mut LowerCtx<'_>) ->
        Result<RenderItem, ResourceUnavailable>
    {
        let state = ctx.materials.get(&self.material);
        let pipeline = Arc::clone(state.pipeline()?);
        let descriptors = Arc::clone(state.desc()?);

        let mesh = resolve_mesh(
            &self.mesh, ctx.resources, pipeline.vertex_layout())?;

        Ok(RenderItem {
            mesh,
            pipeline,
            descriptors,
            instance: self.xform_index,
        })
    }
}

crate fn resolve_mesh(
    mesh: &RenderMesh,
    resources: &ResourceSystem,
    layout: &VertexInputLayout,
) -> Option<MeshData> {
    let index = tryopt! {
        let index = mesh.index()?;
        let buffer = Arc::clone(resources.get_buffer(&index.buffer)?);
        IndexBuffer { ty: index.ty, buffer }
    };

    let attrs = layout.attributes.iter().map(move |binding| {
        let attr = &mesh.bindings()[binding.attribute];
        assert_eq!(attr.format, binding.format);
        let buffer = Arc::clone(resources.get_buffer(&attr.buffer)?);
        Some(AttrBuffer { format: attr.format, buffer })
    }).collect::<Option<_>>()?;

    Some(MeshData { vertex_count: mesh.vertex_count(), index, attrs })
}
