use std::ptr;
use std::sync::Arc;

use derive_more::*;

use crate::*;

#[derive(Debug)]
pub struct Framebuffer {
    pass: Arc<RenderPass>,
    attachments: Vec<AttachmentImage>,
    inner: vk::Framebuffer,
}

#[derive(Debug, From)]
pub enum AttachmentImage {
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
    pub unsafe fn new(
        pass: Arc<RenderPass>,
        // TODO: Should be EnumMap<Attachment, AttachmentImage>
        attachments: Vec<AttachmentImage>,
    ) -> Self {
        create_framebuffer(pass, attachments)
    }

    #[inline]
    pub fn device(&self) -> &Arc<Device> {
        self.pass.device()
    }

    #[inline]
    pub fn inner(&self) -> vk::Framebuffer {
        self.inner
    }

    #[inline]
    pub fn pass(&self) -> &Arc<RenderPass> {
        &self.pass
    }

    #[inline]
    pub fn attachments(&self) -> &[AttachmentImage] {
        &self.attachments
    }

    #[inline]
    pub fn extent(&self) -> Extent2D {
        self.attachments[0].extent()
    }

    #[inline]
    pub fn render_area(&self) -> vk::Rect2D {
        vk::Rect2D::new(vk::Offset2D::new(0, 0), self.extent().into())
    }

    #[inline]
    pub fn viewport(&self) -> vk::Viewport {
        let extent = self.extent();
        vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: extent.width as _,
            height: extent.height as _,
            min_depth: 0.0,
            max_depth: 1.0,
        }
    }

    #[inline]
    pub fn is_swapchain_valid(&self) -> bool {
        self.attachments.iter().all(|attch| attch.is_valid())
    }
}

impl AttachmentImage {
    #[inline]
    pub fn view(&self) -> vk::ImageView {
        match &self {
            Self::Image(view) => view.inner(),
            Self::Swapchain(view) => view.inner(),
        }
    }

    #[inline]
    pub fn extent(&self) -> Extent2D {
        match &self {
            Self::Image(view) => view.extent().to_2d(),
            Self::Swapchain(view) => view.extent(),
        }
    }

    #[inline]
    pub fn format(&self) -> Format {
        match &self {
            Self::Image(view) => view.format(),
            Self::Swapchain(view) => view.format(),
        }
    }

    #[inline]
    pub fn samples(&self) -> SampleCount {
        match &self {
            Self::Image(img) => img.samples(),
            Self::Swapchain(_) => SampleCount::One,
        }
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        if let Self::Swapchain(sw) = self {
            sw.is_valid()
        } else {
            true
        }
    }
}

unsafe fn create_framebuffer(
    render_pass: Arc<RenderPass>,
    attachments: Vec<AttachmentImage>,
) -> Framebuffer {
    let dt = &*render_pass.device().table;

    validate_framebuffer_creation(&render_pass, &attachments);

    let extent = attachments[0].extent();
    let vk_attchs: Vec<_> = attachments.iter().map(|a| a.view()).collect();
    let create_info = vk::FramebufferCreateInfo {
        render_pass: render_pass.inner(),
        attachment_count: vk_attchs.len() as _,
        p_attachments: vk_attchs.as_ptr(),
        width: extent.width,
        height: extent.height,
        layers: 1,
        ..Default::default()
    };
    let mut inner = vk::null();
    dt.create_framebuffer(&create_info, ptr::null(), &mut inner)
        .check()
        .unwrap();

    let framebuffer = Framebuffer {
        pass: render_pass,
        attachments,
        inner,
    };
    assert!(framebuffer.is_swapchain_valid());
    framebuffer
}

fn validate_framebuffer_creation(render_pass: &RenderPass, attachments: &[AttachmentImage]) {
    assert!(!attachments.is_empty());
    assert_eq!(attachments.len(), render_pass.attachments().len());

    let extent = attachments[0].extent();
    for (attch, desc) in attachments.iter().zip(render_pass.attachments().iter()) {
        assert_eq!(attch.format(), desc.format);
        assert_eq!(attch.samples(), desc.samples);

        assert_eq!(attch.extent(), extent);

        if let AttachmentImage::Image(view) = &attch {
            assert_eq!(view.layers(), 1);
            assert_eq!(view.mip_levels(), 1);
        }
    }
}

pub fn create_render_target(
    heap: &ImageHeap,
    render_pass: &Arc<RenderPass>,
    index: usize,
    extent: Extent2D,
    // Boolean args suck but this is pretty low-level anyways
    sampled: bool,
) -> Arc<ImageView> {
    let attch = &render_pass.attachments()[index];
    let mut flags = Default::default();
    if sampled {
        flags |= ImageFlags::NO_SAMPLE
    };
    if attch.format.is_depth_stencil() {
        flags |= ImageFlags::DEPTH_STENCIL_ATTACHMENT;
    } else {
        flags |= ImageFlags::COLOR_ATTACHMENT;
    }
    if render_pass.is_input_attachment(index) {
        flags |= ImageFlags::INPUT_ATTACHMENT;
    }
    Arc::new(Image::with(
        &heap,
        flags,
        ImageType::Dim2,
        attch.format,
        attch.samples,
        extent.into(),
        1,
        1,
    ))
    .create_full_view()
}

#[cfg(test)]
pub unsafe fn create_test_framebuffer(swapchain: &Swapchain) {
    let device = Arc::clone(swapchain.device());
    let heap = ImageHeap::new(device);

    let pass = create_test_pass(swapchain.device());

    let extent = swapchain.extent;
    let hdr = create_render_target(&heap, &pass, 1, extent, false);
    let depth = create_render_target(&heap, &pass, 2, extent, false);
    let normal = create_render_target(&heap, &pass, 3, extent, false);
    let albedo = create_render_target(&heap, &pass, 4, extent, false);

    let views = swapchain.create_views();
    let back = Arc::clone(&views[0]);

    let _fb = Framebuffer::new(
        pass,
        vec![
            back.into(),
            hdr.into(),
            depth.into(),
            normal.into(),
            albedo.into(),
        ],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    unsafe fn create(vars: testing::TestVars) {
        let _fb = create_test_framebuffer(vars.swapchain());
    }
}
