#![allow(unused_imports)]
use std::mem::ManuallyDrop;
use std::ptr;
use std::sync::Arc;

use derive_more::*;

use crate::*;

#[derive(Debug)]
crate struct Framebuffer {
    pass: Arc<RenderPass>,
    attachments: Vec<Attachment>,
    inner: vk::Framebuffer,
}

#[derive(Debug, From)]
crate enum Attachment {
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

    crate fn viewport(&self) -> vk::Viewport {
        let extent = self.extent();
        vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: extent.width as _,
            height: extent.height as _,
            // Note how the depth is reversed
            min_depth: 1.0,
            max_depth: 0.0,
        }
    }
}

impl Attachment {
    crate fn view(&self) -> vk::ImageView {
        match &self {
            Self::Image(view) => view.inner(),
            Self::Swapchain(view) => view.inner(),
        }
    }

    crate fn extent(&self) -> Extent2D {
        match &self {
            Self::Image(view) => view.extent().into(),
            Self::Swapchain(view) => view.swapchain().extent,
        }
    }

    crate fn format(&self) -> Format {
        match &self {
            Self::Image(view) => view.format(),
            Self::Swapchain(view) => view.swapchain().format(),
        }
    }

    crate fn samples(&self) -> SampleCount {
        match &self {
            Self::Image(img) => img.samples(),
            Self::Swapchain(_) => SampleCount::One,
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
    assert!(attachments.len() > 0);
    assert_eq!(attachments.len(), render_pass.attachments().len());

    let extent = attachments[0].extent();
    for (attch, desc) in attachments.iter()
        .zip(render_pass.attachments().iter())
    {
        assert_eq!(attch.format(), desc.format);
        assert_eq!(attch.samples(), desc.samples);

        assert_eq!(attch.extent(), extent);

        if let Attachment::Image(view) = &attch {
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
        )).create_full_view()
    }
}

#[cfg(test)]
crate unsafe fn create_test_framebuffer(swapchain: &Arc<Swapchain>) {
    use AttachmentName::*;

    let device = Arc::clone(&swapchain.device);
    let state = Arc::new(SystemState::new(device));
    let get_state = || Arc::clone(&state);

    let pass = create_test_pass(Arc::clone(&swapchain.device));

    let extent = swapchain.extent;
    let hdr = create_render_target(get_state(), &pass, 1, extent, false);
    let depth = create_render_target(get_state(), &pass, 2, extent, false);
    let normal = create_render_target(get_state(), &pass, 3, extent, false);
    let albedo = create_render_target(get_state(), &pass, 4, extent, false);

    let views = swapchain.create_views();
    let back = Arc::clone(&views[0]);

    let _fb = Framebuffer::new(pass, vec![
        back.into(),
        hdr.into(),
        depth.into(),
        normal.into(),
        albedo.into(),
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
