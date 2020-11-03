use std::borrow::Cow;
use std::sync::Arc;

use device::{
    BufferBinding, BufferBox, DescriptorSet, DescriptorSetLayout,
    SetLayoutCache,
};
use math::Matrix4;

use crate::*;

#[derive(Debug)]
pub struct RenderWorld {
    crate rloop: Box<RenderLoop>,
    crate data: RenderWorldData,
}

#[derive(Debug, Default)]
pub struct WorldUniforms {
    pub view: SceneView,
    pub xforms: Vec<Matrix4>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
enum Binding {
    ViewUniforms = 0,
    XformBuffer = 1,
}

#[derive(Debug, Default)]
crate struct RenderWorldData {
    crate objects: Vec<RenderObject>,
    crate uniforms: WorldUniforms,
}

impl RenderWorld {
    #[inline]
    pub fn new(rloop: Box<RenderLoop>) -> Self {
        Self {
            rloop,
            data: Default::default(),
        }
    }

    #[inline]
    pub fn render_loop(&self) -> &RenderLoop {
        &self.rloop
    }

    #[inline]
    pub fn render_loop_mut(&mut self) -> &mut RenderLoop {
        &mut self.rloop
    }

    #[inline]
    pub fn into_inner(self) -> Box<RenderLoop> {
        self.rloop
    }

    #[inline]
    pub fn frame_num(&self) -> u64 {
        self.rloop.frame_num()
    }

    #[inline]
    pub fn uniforms(&self) -> &WorldUniforms {
        &self.data.uniforms
    }

    #[inline]
    pub fn set_uniforms(&mut self, uniforms: WorldUniforms) {
        self.data.uniforms = uniforms;
    }

    #[inline]
    pub fn add_object(&mut self, obj: impl Into<RenderObject>) {
        self.data.objects.push(obj.into());
    }

    pub fn render(self) -> Box<RenderLoop> {
        let mut rloop = self.rloop;
        let world = self.data;
        rloop.render(world);
        rloop
    }
}

impl WorldUniforms {
    crate fn create_set_layout(layouts: &SetLayoutCache) ->
        Cow<Arc<DescriptorSetLayout>>
    {
        layouts.get_or_create_named(&device::set_layout_desc![
            (Binding::ViewUniforms as _, UniformBuffer,
                VERTEX_BIT | FRAGMENT_BIT),
            (Binding::XformBuffer as _, StorageBuffer, VERTEX_BIT),
        ], Some("scene_descriptors_layout"))
    }

    fn write_descriptors(
        &self,
        state: &SystemState,
        desc: &mut DescriptorSet,
    ) {
        let view = SceneViewUniforms::new(&self.view);
        let view = state.buffers.boxed(
            BufferBinding::Uniform, Lifetime::Frame, view);
        let range = BufferBox::range(&view);
        desc.write_buffer(Binding::ViewUniforms as _, range);

        let xforms = state.buffers.box_slice(
            BufferBinding::Storage, Lifetime::Frame, &self.xforms);
        let range = BufferBox::range(&xforms);
        desc.write_buffer(Binding::XformBuffer as _, range);
    }

    crate fn create_descriptor_set(&self, state: &SystemState) -> DescriptorSet
    {
        let layout = Self::create_set_layout(&state.set_layouts);
        let mut desc = state.descriptors.alloc(
            Lifetime::Frame,
            layout.as_ref(),
        );
        desc.set_name("world_uniforms");
        self.write_descriptors(state, &mut desc);
        desc
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use super::*;

    unsafe fn render_nothing(vars: crate::testing::TestVars) {
        let window = Arc::clone(&vars.swapchain.window());
        let app_info = (*vars.device().instance().app_info()).clone();
        let rl = Box::new(RenderLoop::new(app_info, window).unwrap());
        let world = RenderWorld::new(rl);
        world.render();
    }

    unit::declare_tests![render_nothing];
}

unit::collect_tests![tests];
