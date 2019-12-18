use enum_map::Enum;

#[derive(Clone, Copy, Debug, Enum, Eq, Hash, PartialEq)]
crate enum AttachmentName {
    Color,
    Depth,
    DepthStencil,
    // G-buffer components etc.
}

/*
use std::ptr;
use std::sync::Arc;

use crate::*;

#[derive(Debug)]
crate struct Attachment {
    device: Arc<Device>,
    image: ImageInfo,
    owned_image: bool,
    view: vk::ImageView,
}

#[derive(Debug)]
crate struct Framebuffer {
    device: Arc<Device>,
    pass: Arc<RenderPass>,
    attachments: Vec<Arc<Attachment>>,
    inner: vk::Framebuffer,
}

impl Drop for Attachment {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            dt.destroy_image_view(self.view, ptr::null());
            if self.owned_image {
                dt.destroy_image(self.image, ptr::null());
            }
        }
    }
}

impl Attachment {
    crate unsafe fn from_image(device: Arc<Device>, info: ImageInfo) -> Self {
        let view = create_image_view(&device, &info);
        Attachment {
            device,
            image: info,
            owned_image: false,
            view,
        }
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    crate fn view(&self) -> vk::ImageView {
        self.view
    }

    crate fn extent(&self) -> vk::Extent2D {
        let extent = self.info.extent;
        vk::Extent2D::new(extent.width, extent.height)
    }

    crate fn format(&self) -> vk::Format {
        self.info.format
    }

    crate fn samples(&self) -> vk::SampleCountFlags {
        self.info.samples
    }

    crate fn layers(&self) -> u32 {
        self.info.layers
    }

    crate fn mip_levels(&self) -> u32 {
        self.info.mip_levels
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            dt.destroy_framebuffer(self.inner, ptr::null());
        }
    }
}

unsafe fn create_framebuffer(
    render_pass: Arc<RenderPass>,
    attachments: Vec<Arc<Attachment>>,
) -> Framebuffer {
    let device: Arc<Device> = Arc::clone(render_pass.device());
    let dt = &*device.table;

    assert_eq!(attachments.len(), render_pass.attachments().len());
    for (attch, desc) in attachments.iter()
        .zip(render_pass.attachments().iter())
    {
        assert_eq!(attch.format, desc.format);
        assert_eq!(attch.samples, desc.samples);
    }

    let extent = attachments[0].extent;
    let layers = attachments[0].layers;
    for attch in attachments.iter() {
        assert_eq!(attch.extent, extent);
        assert_eq!(attch.layers, layers);
    }

    let vk_attachments: Vec<_> = attachments.iter().map(|a| a.view).collect();
    let create_info = vk::FramebufferCreateInfo {
        render_pass: render_pass.inner(),
        attachment_count: vk_attachments.len() as _,
        p_attachments: vk_attachments.as_ptr(),
        width: extent.width,
        height: extent.height,
        layers,
        ..Default::default()
    };
    let mut inner = vk::null();
    dt.create_framebuffer(&create_info, ptr::null(), &mut inner)
        .check().unwrap();

    Framebuffer {
        device,
        pass: Arc::clone(&render_pass),
        attachments,
        inner,
    }
}

impl Framebuffer {
    crate unsafe fn new(
        render_pass: Arc<RenderPass>,
        attachments: Vec<Arc<Attachment>>,
    ) -> Self {
        create_framebuffer(render_pass, attachments)
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    crate fn inner(&self) -> vk::Framebuffer {
        self.inner
    }

    crate fn pass(&self) -> &Arc<RenderPass> {
        &self.pass
    }

    crate fn attachments(&self) -> &[Arc<Attachment>] {
        &self.attachments
    }

    crate fn extent(&self) -> vk::Extent2D {
        self.attachments[0].extent
    }

    crate fn render_area(&self) -> vk::Rect2D {
        vk::Rect2D::new(vk::Offset2D::new(0, 0), self.extent())
    }
}
*/
