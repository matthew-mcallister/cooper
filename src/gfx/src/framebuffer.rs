#![allow(unused_imports)]
use std::mem::ManuallyDrop;
use std::ptr;
use std::sync::Arc;

use derive_more::*;
use enum_map::Enum;

use crate::*;

// TODO: Get rid of this type?
#[derive(Clone, Copy, Debug, Enum, Eq, Hash, PartialEq)]
crate enum AttachmentName {
    /// SRGB screen buffer
    Backbuffer,
    DepthStencil,
    /// HDR light buffer
    Hdr,
    Normal,
    Albedo,
}

#[derive(Debug)]
crate struct Framebuffer {
    pass: Arc<RenderPass>,
    attachments: Vec<Attachment>,
    inner: vk::Framebuffer,
}

#[derive(Debug)]
crate struct Attachment {
    crate name: AttachmentName,
    crate data: AttachmentData,
}

#[derive(Debug, From)]
crate enum AttachmentData {
    Image(Arc<ImageView>),
    Swapchain(Arc<SwapchainView>),
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        let dt = &*self.pass.device().table;
        unsafe {
            dt.destroy_framebuffer(self.inner, ptr::null());
        }
    }
}

impl Framebuffer {
    crate unsafe fn new(
        pass: Arc<RenderPass>,
        attachments: Vec<Attachment>,
    ) -> Self {
        create_framebuffer(pass, attachments)
    }

    crate fn device(&self) -> &Arc<Device> {
        self.pass.device()
    }

    crate fn inner(&self) -> vk::Framebuffer {
        self.inner
    }

    crate fn pass(&self) -> &Arc<RenderPass> {
        &self.pass
    }

    crate fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    crate fn extent(&self) -> Extent2D {
        self.attachments[0].extent()
    }

    crate fn render_area(&self) -> vk::Rect2D {
        vk::Rect2D::new(vk::Offset2D::new(0, 0), self.extent().into())
    }
}

impl Attachment {
    crate fn new<T: Into<AttachmentData>>(name: AttachmentName, data: T) ->
        Self
    {
        Self {
            name,
            data: data.into(),
        }
    }

    crate fn view(&self) -> vk::ImageView {
        use AttachmentData::*;
        match &self.data {
            Image(view) => view.inner(),
            Swapchain(view) => view.inner(),
        }
    }

    crate fn extent(&self) -> Extent2D {
        use AttachmentData::*;
        match &self.data {
            Image(view) => view.extent().into(),
            Swapchain(view) => view.swapchain().extent,
        }
    }

    crate fn format(&self) -> Format {
        use AttachmentData::*;
        match &self.data {
            Image(view) => view.format(),
            Swapchain(view) => view.swapchain().format(),
        }
    }

    crate fn samples(&self) -> SampleCount {
        use AttachmentData::*;
        match &self.data {
            Image(img) => img.samples(),
            Swapchain(_) => SampleCount::One,
        }
    }
}

unsafe fn create_framebuffer(
    render_pass: Arc<RenderPass>,
    attachments: Vec<Attachment>,
) -> Framebuffer {
    let dt = &*render_pass.device().table;

    validate_framebuffer_creation(&render_pass, &attachments);

    let extent = attachments[0].extent();
    let vk_attachments: Vec<_> = attachments.iter().map(|a| a.view())
        .collect();
    let create_info = vk::FramebufferCreateInfo {
        render_pass: render_pass.inner(),
        attachment_count: vk_attachments.len() as _,
        p_attachments: vk_attachments.as_ptr(),
        width: extent.width,
        height: extent.height,
        layers: 1,
        ..Default::default()
    };
    let mut inner = vk::null();
    dt.create_framebuffer(&create_info, ptr::null(), &mut inner)
        .check().unwrap();

    Framebuffer {
        pass: render_pass,
        attachments,
        inner,
    }
}

fn validate_framebuffer_creation(
    render_pass: &RenderPass,
    attachments: &[Attachment],
) {
    use AttachmentData::*;

    assert!(attachments.len() > 0);
    assert_eq!(attachments.len(), render_pass.attachments().len());

    let extent = attachments[0].extent();
    for (attch, desc) in attachments.iter()
        .zip(render_pass.attachments().iter())
    {
        assert_eq!(attch.format(), desc.format);
        assert_eq!(attch.samples(), desc.samples);

        assert_eq!(attch.extent(), extent);

        if let Image(view) = &attch.data {
            assert_eq!(view.layers(), 1);
            assert_eq!(view.mip_levels(), 1);
        }
    }
}

crate fn create_render_target(
    state: Arc<SystemState>,
    render_pass: &Arc<RenderPass>,
    index: usize,
    extent: Extent2D,
    // Boolean args suck but this is pretty low-level anyways
    sampled: bool,
) -> Arc<ImageView> {
    let attch = &render_pass.attachments()[index];
    let mut flags = Default::default();
    if sampled { flags |= ImageFlags::NO_SAMPLE };
    if attch.format.is_depth_stencil() {
        flags |= ImageFlags::DEPTH_STENCIL_ATTACHMENT
    } else {
        flags |= ImageFlags::COLOR_ATTACHMENT
    }
    if render_pass.is_input_attachment(index) {
        flags |= ImageFlags::INPUT_ATTACHMENT
    }
    unsafe {
        Arc::new(Image::new(
            state,
            flags,
            ImageType::TwoDim,
            attch.format,
            attch.samples,
            extent.into(),
            1,
            1,
        )).create_default_view()
    }
}

#[cfg(test)]
crate unsafe fn create_test_framebuffer(swapchain: &Arc<Swapchain>) {
    use AttachmentName::*;

    let device = Arc::clone(&swapchain.device);
    let state = Arc::new(SystemState::new(device));

    let pass = create_test_pass(Arc::clone(&swapchain.device));

    let extent = swapchain.extent;
    let hdr =
        create_render_target(Arc::clone(&state), &pass, 1, extent, false);
    let depth =
        create_render_target(Arc::clone(&state), &pass, 2, extent, false);
    let normal =
        create_render_target(Arc::clone(&state), &pass, 3, extent, false);
    let albedo =
        create_render_target(Arc::clone(&state), &pass, 4, extent, false);

    let views = swapchain.create_views();
    let back = Arc::clone(&views[0]);

    let _fb = Framebuffer::new(pass, vec![
        Attachment::new(Backbuffer, back),
        Attachment::new(Hdr, hdr),
        Attachment::new(DepthStencil, depth),
        Attachment::new(Normal, normal),
        Attachment::new(Albedo, albedo),
    ]);
}

#[cfg(test)]
mod tests {
    use crate::*;
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let _fb = create_test_framebuffer(&vars.swapchain);
    }

    unit::declare_tests![smoke_test];
}

unit::collect_tests![tests];
