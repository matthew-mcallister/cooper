use std::mem::MaybeUninit;
use std::sync::Arc;

use device::{BufferBinding, BufferBox, Lifetime};
use log::debug;
use more_asserts::assert_gt;
use prelude::*;

use crate::{RenderMesh, ResourceUnavailable, SystemState};
use crate::material::{Material, MaterialSystem};
use crate::render::{PerInstanceData, SceneDescriptors, SceneViewUniforms};
use crate::resource::ResourceSystem;
use super::*;

// TODO: This is going to create a lot of work managing refcounts
#[derive(Debug)]
crate struct RenderItem {
    crate mesh: Arc<RenderMesh>,
    // TODO: Possibly lower Material to a more primitive representation
    crate material: Arc<Material>,
}

#[derive(Debug)]
struct LowerCtx<'ctx> {
    state: &'ctx SystemState,
    uniforms: &'ctx SceneViewUniforms,
    resources: &'ctx ResourceSystem,
    materials: &'ctx mut MaterialSystem,
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
        materials: &'ctx mut MaterialSystem,
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
    materials: &'a mut MaterialSystem,
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
    fn lower<'ctx>(self, instance: u32, ctx: &mut LowerCtx<'ctx>) ->
        Result<RenderItem, ResourceUnavailable>
    {
        match self {
            RenderObject::MeshInstance(mesh) => mesh.lower(instance, ctx),
        }
    }
}

impl Lower for MeshInstance {
    fn lower<'ctx>(self, instance: u32, ctx: &mut LowerCtx<'ctx>) ->
        Result<RenderItem, ResourceUnavailable>
    {
        let xform = ctx.uniforms.view * self.xform();
        ctx.instance_data[instance as usize].set_xform(xform);

        let material = Arc::clone(ctx.materials.get_or_create(
            ctx.state,
            ctx.resources,
            &self.material,
        )?);

        Ok(RenderItem {
            mesh: self.mesh,
            material,
        })
    }
}
