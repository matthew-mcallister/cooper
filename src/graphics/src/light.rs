use std::marker::PhantomData;

use crate::*;

// TODO: Find a way to pack this into 16 bytes?
#[repr(align = "16")]
#[derive(Copy, Clone, Debug)]
pub struct PointLight {
    pub pos: [f32; 4],
    pub color_radius: [f32; 4],
}

/// This type provides a static light acceleration structure for point
/// lights.
// TODO: serialization
#[derive(Debug)]
pub struct StaticLightAccel<B: Backend> {
    lights: Vec<PointLight>,
    _marker: PhantomData<B>,
}

impl<B: Backend> StaticLightAccel<B> {
    pub unsafe fn new(lights: Vec<PointLight>) -> Self {
        StaticLightAccel { lights }
    }
}

/// This type provides a coarse-grained, mutable light acceleration
/// structure for moving lights that is independent of the view frustum.
#[derive(Debug)]
pub struct DynamicLightAccel<B: Backend> {
    lights: Vec<PointLight>,
    _marker: PhantomData<B>,
}

impl<B: Backend> DynamicLightAccel<B> {
    pub unsafe fn new(lights: Vec<PointLight>) -> Self {
        DynamicLightAccel { lights }
    }
}

/// This type creates a fine-grained, per-frame light acceleration
/// structure for finding interacting lights in the fragment shader.
#[derive(Debug)]
pub struct FrameLightAccel<B: Backend> {
    uniforms: B::UboChain,
    cur_buffer: B::Ubo,
}

#[derive(Debug)]
impl<B: Backend> FrameLights<B> {
    pub unsafe fn new(backend: &mut Backend) -> Self {
        FrameLightAccel {
            uniforms: B::UboChain::new_array::<PointLight>(backend),
        }
    }

    pub unsafe fn new_frame(
        &mut self,
        frustum: Frustum,
        statik: StaticLightAccel,
        dynamic: DynamicLightAccel,
    ) {
        let array_len = uniforms.buffer_len();
        let total_lights = statik.len() + dynamic.len();
        assert!(total_lights < uniforms.buffer_len());

        self.cur_buffer = self.uniforms.wait_for_next();
        let slice: &mut [PointLight] = cast_bytes(&*buf.slice());
        let iter = slice.iter_mut();

        for (dst, &light) in iter.zip(statik.iter().chain(dynamic.iter())) {
            *dst = light;
        }
    }
}
