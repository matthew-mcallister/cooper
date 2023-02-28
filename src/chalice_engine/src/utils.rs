use std::sync::Arc;

use crate::Engine;

#[derive(Debug, Copy, Clone, Default)]
pub struct FramebufferImageInfo<'s> {
    pub flags: device::ImageFlags,
    pub format: device::Format,
    pub samples: device::SampleCount,
    pub name: Option<&'s str>,
}

/// Provides a shortcut for creating images to use for rendering to the
/// screen. Namely, your G-buffer(s), depth-stencil buffer(s), and
/// multisample/HDR color buffers.
pub fn create_framebuffer_images(
    engine: &Engine,
    infos: &[FramebufferImageInfo<'_>],
) -> Vec<Arc<device::Image>> {
    infos
        .iter()
        .map(|info| {
            let mut def = device::ImageDef::new(
                engine.device(),
                info.flags,
                device::ImageType::Dim2,
                info.format,
                info.samples,
                engine.swapchain().extent().into(),
                1,
                1,
            );
            if let Some(s) = info.name {
                def.set_name(s.to_owned());
            }
            Arc::new(device::Image::new(engine.image_heap(), Arc::new(def)))
        })
        .collect()
}
