use std::sync::Arc;

use device::{
    BufferRange, DescriptorSet, DescriptorSetLayout, Lifetime, SetLayoutCache,
};
use math::{Matrix4, Matrix4x3};

use crate::{Globals, SystemState};
use crate::util::pack_xform;

#[derive(Clone, Copy, Debug, Default)]
#[repr(C, align(16))]
crate struct PerInstanceData {
    crate xform: Matrix4x3,
}

impl PerInstanceData {
    crate fn set_xform(&mut self, xform: Matrix4) {
        self.xform = pack_xform(xform);
    }
}

#[derive(Debug)]
crate struct SceneDescriptors {
    inner: DescriptorSet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
enum Binding {
    ViewUniforms = 0,
    InstanceUniforms = 1,
}

impl SceneDescriptors {
    crate fn create_layout(layouts: &SetLayoutCache) ->
        Arc<DescriptorSetLayout>
    {
        layouts.get_or_create_named(&device::set_layout_desc![
            (Binding::ViewUniforms as _, UniformBuffer,
                VERTEX_BIT | FRAGMENT_BIT),
            (Binding::InstanceUniforms as _, StorageBuffer, VERTEX_BIT),
        ], Some("scene_descriptors")).into_owned()
    }

    pub(super) fn new(state: &SystemState, globals: &Globals) -> Self {
        let mut inner = state.descriptors.alloc(
            Lifetime::Frame,
            &globals.scene_desc_layout,
        );
        inner.set_name("scene_descriptors.inner");
        SceneDescriptors { inner }
    }

    pub(super) fn inner(&self) -> &DescriptorSet {
        &self.inner
    }

    crate fn write_view_uniforms(&mut self, buffer: BufferRange<'_>) {
        self.inner.write_buffer(Binding::ViewUniforms as _, buffer);
    }

    crate fn write_instance_uniforms(&mut self, buffer: BufferRange<'_>) {
        self.inner.write_buffer(Binding::InstanceUniforms as _, buffer);
    }
}
