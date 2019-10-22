use std::ptr;
use std::sync::Arc;

use ccore::name::*;

use crate::*;

#[derive(Debug)]
pub struct AttachmentChain {
    pub device: Arc<Device>,
    pub views: Vec<vk::ImageView>,
    pub extent: vk::Extent2D,
    pub format: vk::Format,
    pub samples: vk::SampleCountFlags,
    pub layers: u32,
}

impl Drop for AttachmentChain {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            for &view in self.views.iter() {
                dt.destroy_image_view(view, ptr::null());
            }
        }
    }
}

impl AttachmentChain {
    pub unsafe fn from_swapchain(swapchain: &Swapchain) -> Self {
        let device = Arc::clone(&swapchain.device);
        let extent = swapchain.extent;
        let extent = vk::Extent2D::new(extent.width, extent.height);
        let format = swapchain.format;
        let samples = vk::SampleCountFlags::_1_BIT;
        let layers = 1;
        let views = swapchain.images.iter().map(|&image| {
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
            view
        }).collect();
        AttachmentChain {
            device,
            views,
            extent,
            format,
            samples,
            layers,
        }
    }

    pub fn len(&self) -> usize {
        self.views.len()
    }
}

#[derive(Debug)]
pub struct FramebufferChain {
    pub device: Arc<Device>,
    pub pass: Name,
    pub extent: vk::Extent2D,
    pub attachments: Vec<Arc<AttachmentChain>>,
    pub framebuffers: Vec<vk::Framebuffer>,
}

impl Drop for FramebufferChain {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            for &framebuffer in self.framebuffers.iter() {
                dt.destroy_framebuffer(framebuffer, ptr::null());
            }
        }
    }
}

unsafe fn create_framebuffer(
    core: &CoreData,
    render_pass: Name,
    attachments: Vec<Arc<AttachmentChain>>,
) -> FramebufferChain {
    let device = Arc::clone(core.device());
    let dt = &*device.table;
    let render_pass_id = render_pass;
    let render_pass = core.get_pass(render_pass_id);

    assert_eq!(attachments.len(), render_pass.attachments().len());
    for (attachment, desc) in attachments.iter()
        .zip(render_pass.attachments().iter())
    {
        assert_eq!(attachment.format, desc.format);
        assert_eq!(attachment.samples, desc.samples);
    }

    let len = attachments[0].len();
    let extent = attachments[0].extent;
    let layers = attachments[0].layers;
    for chain in attachments.iter() {
        assert_eq!(chain.len(), len);
        assert_eq!(chain.extent, extent);
        assert_eq!(chain.layers, layers);
    }

    let framebuffers: Vec<_> = (0..len).map(|idx| {
        let attachments: Vec<_> = attachments.iter()
            .map(|a| a.views[idx])
            .collect();
        let create_info = vk::FramebufferCreateInfo {
            render_pass: render_pass.inner(),
            attachment_count: attachments.len() as _,
            p_attachments: attachments.as_ptr(),
            width: extent.width,
            height: extent.height,
            layers,
            ..Default::default()
        };
        let mut framebuffer = vk::null();
        dt.create_framebuffer(&create_info, ptr::null(), &mut framebuffer)
            .check().unwrap();

        framebuffer
    }).collect();

    FramebufferChain {
        device,
        pass: render_pass_id,
        extent,
        attachments,
        framebuffers,
    }
}

impl FramebufferChain {
    pub unsafe fn new(
        core: &CoreData,
        render_pass: Name,
        attachments: Vec<Arc<AttachmentChain>>,
    ) -> Self {
        create_framebuffer(core, render_pass, attachments)
    }

    pub fn len(&self) -> usize {
        self.framebuffers.len()
    }

    pub fn rect(&self) -> vk::Rect2D {
        vk::Rect2D::new(vk::Offset2D::new(0, 0), self.extent)
    }
}
