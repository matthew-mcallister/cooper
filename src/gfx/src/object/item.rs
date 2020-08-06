use std::mem::MaybeUninit;
use std::sync::Arc;

use device::{
    BufferAlloc, BufferBinding, BufferBox, DescriptorSet, GraphicsPipeline,
    Lifetime, VertexInputLayout,
};
use log::debug;
use more_asserts::assert_gt;
use prelude::*;

use crate::{ResourceUnavailable, SystemState};
use crate::material::MaterialStateTable;
use crate::mesh::{AttrBuffer, IndexBuffer, RenderMesh};
use crate::render::{PerInstanceData, SceneDescriptors, SceneViewUniforms};
use crate::resource::ResourceSystem;
use crate::util::SmallVec;
use super::*;

#[derive(Debug)]
crate struct RenderItem {
    crate mesh: MeshData,
    crate pipeline: Arc<GraphicsPipeline>,
    crate descriptors: Arc<DescriptorSet>,
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
    uniforms: &'ctx SceneViewUniforms,
    resources: &'ctx ResourceSystem,
    materials: &'ctx MaterialStateTable,
    instance_data: &'static mut [PerInstanceData],
}

trait Lower {
    fn lower<'ctx>(self, instance: u32, ctx: &mut LowerCtx<'ctx>) ->
        Result<RenderItem, ResourceUnavailable>;
}

impl<'ctx> LowerCtx<'ctx> {
    fn new<'obj>(
        state: &'ctx SystemState,
        uniforms: &'ctx SceneViewUniforms,
        resources: &'ctx ResourceSystem,
        materials: &'ctx MaterialStateTable,
        descs: &'ctx mut SceneDescriptors,
        object_count: usize,
    ) -> Self {
        assert_gt!(object_count, 0);

        let instance_buf = state.buffers.box_uninit(
            BufferBinding::Storage,
            Lifetime::Frame,
            object_count,
        );
        descs.write_instance_uniforms(BufferBox::range(&instance_buf));

        // TODO: 99% sure lifetimes can be used to ensure that there are
        // no dangling pointers like this one at the end of a frame
        let instance_data = unsafe {
            let slice = &mut *BufferBox::leak(instance_buf).as_ptr();
            MaybeUninit::slice_get_mut(slice)
        };

        Self { state, uniforms, resources, materials, instance_data }
    }
}

crate fn lower_objects<'a>(
    state: &'a SystemState,
    uniforms: &'a SceneViewUniforms,
    resources: &'a ResourceSystem,
    materials: &'a MaterialStateTable,
    descs: &'a mut SceneDescriptors,
    objects: impl ExactSizeIterator<Item = RenderObject> + 'a,
) -> impl Iterator<Item = RenderItem> + 'a {
    let mut ctx = LowerCtx::new(
        state, uniforms, resources, materials, descs, objects.len());
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
    fn lower(self, instance: u32, ctx: &mut LowerCtx<'_>) ->
        Result<RenderItem, ResourceUnavailable>
    {
        let xform = ctx.uniforms.view * self.xform();
        ctx.instance_data[instance as usize].set_xform(xform);

        let state = ctx.materials.get(&self.material);
        let pipeline = Arc::clone(state.pipeline()?);
        let descriptors = Arc::clone(state.desc()?);

        let mesh = resolve_mesh(
            &self.mesh, ctx.resources, pipeline.vertex_layout())?;

        Ok(RenderItem {
            mesh,
            pipeline,
            descriptors,
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
