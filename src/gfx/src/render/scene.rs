use std::sync::Arc;

use math::{Matrix4, Matrix4x3};

use crate::{Globals, SystemState};
use crate::device::{
    BufferRange, DescriptorSet, DescriptorSetLayout, Device, Lifetime,
};
use crate::util::pack_xform;

#[derive(Clone, Copy, Debug, Default)]
#[repr(C, align(16))]
crate struct PerInstanceData {
    crate xform: Matrix4x3<f32>,
}

impl PerInstanceData {
    crate fn set_xform(&mut self, xform: Matrix4<f32>) {
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
    crate fn create_layout(device: Arc<Device>) -> Arc<DescriptorSetLayout> {
        let bindings = [
            vk::DescriptorSetLayoutBinding {
                binding: Binding::ViewUniforms as _,
                descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::VERTEX_BIT
                    | vk::ShaderStageFlags::FRAGMENT_BIT,
                ..Default::default()
            },
            vk::DescriptorSetLayoutBinding {
                binding: Binding::InstanceUniforms as _,
                descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::VERTEX_BIT,
                ..Default::default()
            },
        ];
        let mut layout =
            unsafe { DescriptorSetLayout::from_bindings(device, &bindings) };
        layout.set_name("scene_descriptors");
        Arc::new(layout)
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

    pub(super) fn layout(&self) -> &Arc<DescriptorSetLayout> {
        self.inner.layout()
    }

    crate fn write_view_uniforms(&mut self, buffer: BufferRange<'_>) {
        self.inner.write_buffer(Binding::ViewUniforms as _, buffer);
    }

    crate fn write_instance_uniforms(&mut self, buffer: BufferRange<'_>) {
        self.inner.write_buffer(Binding::InstanceUniforms as _, buffer);
    }
}
