use std::ptr;
use std::sync::Arc;

use ccore::Name;

use crate::*;

// TODO: Attachments should probably be assigned a value that designates
// their purpose, i.e. color, depth, or some g-buffer component.
#[derive(Debug)]
crate struct Attachment {
    device: Arc<Device>,
    view: vk::ImageView,
    extent: vk::Extent2D,
    format: vk::Format,
    samples: vk::SampleCountFlags,
    layers: u32,
}

impl Drop for Attachment {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            dt.destroy_image_view(self.view, ptr::null());
        }
    }
}

crate unsafe fn attachments_from_swapchain(swapchain: &Swapchain) ->
    impl Iterator<Item = Attachment> + '_
{
    let device = Arc::clone(&swapchain.device);
    let extent = swapchain.extent;
    let format = swapchain.format;
    let samples = vk::SampleCountFlags::_1_BIT;
    let layers = 1;
    swapchain.images.iter().map(move |&image| {
        // TODO: Encapsulate images in an "Image" type and make this
        // block a generic "from_image" function
        let create_info = vk::ImageViewCreateInfo {
            image,
            view_type: vk::ImageViewType::_2D,
            format,
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR_BIT,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            },
            ..Default::default()
        };
        let mut view = vk::null();
        device.table.create_image_view
            (&create_info, ptr::null(), &mut view).check().unwrap();
        Attachment {
            device: Arc::clone(&device),
            view,
            extent,
            format,
            samples,
            layers,
        }
    })
}

impl Attachment {
    crate unsafe fn from_swapchain(swapchain: &Swapchain) ->
        impl Iterator<Item = Self> + '_
    {
        attachments_from_swapchain(swapchain)
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    crate fn view(&self) -> vk::ImageView {
        self.view
    }

    crate fn extent(&self) -> vk::Extent2D {
        self.extent
    }

    crate fn format(&self) -> vk::Format {
        self.format
    }

    crate fn samples(&self) -> vk::SampleCountFlags {
        self.samples
    }

    crate fn layers(&self) -> u32 {
        self.layers
    }
}

#[derive(Debug)]
crate struct Framebuffer {
    device: Arc<Device>,
    pass: Arc<RenderPass>,
    attachments: Vec<Arc<Attachment>>,
    inner: vk::Framebuffer,
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
