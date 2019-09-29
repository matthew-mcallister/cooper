use std::ptr;
use std::sync::Arc;

use crate::*;

#[derive(Debug)]
pub struct RenderPass {
    pub device: Arc<Device>,
    pub inner: vk::RenderPass,
    pub attachments: Vec<vk::AttachmentDescription>,
}

impl Drop for RenderPass {
    fn drop(&mut self) {
        let dt = &self.device.table;
        unsafe {
            dt.destroy_render_pass(self.inner, ptr::null());
        }
    }
}

impl RenderPass {
    pub unsafe fn new(
        device: Arc<Device>,
        create_info: &vk::RenderPassCreateInfo,
    ) -> RenderPass {
        let dt = &*device.table;

        let attachments = std::slice::from_raw_parts
            (create_info.p_attachments, create_info.attachment_count as _);
        let attachments = attachments.to_vec();

        let mut render_pass = vk::null();
        dt.create_render_pass
            (create_info as _, ptr::null(), &mut render_pass as _)
            .check().unwrap();

        RenderPass {
            device,
            inner: render_pass,
            attachments,
        }
    }
}

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
                (&create_info as _, ptr::null(), &mut view as _)
                .check().unwrap();
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
    pub render_pass: Arc<RenderPass>,
    pub attachments: Vec<Arc<AttachmentChain>>,
    pub framebuffers: Vec<vk::Framebuffer>,
}

impl Drop for FramebufferChain {
    fn drop(&mut self) {
        let dt = &*self.render_pass.device.table;
        unsafe {
            for &framebuffer in self.framebuffers.iter() {
                dt.destroy_framebuffer(framebuffer, ptr::null());
            }
        }
    }
}

impl FramebufferChain {
    pub unsafe fn new(
        render_pass: Arc<RenderPass>,
        attachments: Vec<Arc<AttachmentChain>>,
    ) -> Self {
        assert_eq!(attachments.len(), render_pass.attachments.len());
        for (attachment, desc) in attachments.iter()
            .zip(render_pass.attachments.iter())
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
                render_pass: render_pass.inner,
                attachment_count: attachments.len() as _,
                p_attachments: attachments.as_ptr(),
                width: extent.width,
                height: extent.height,
                layers,
                ..Default::default()
            };
            let mut framebuffer = vk::null();
            render_pass.device.table.create_framebuffer
                (&create_info as _, ptr::null(), &mut framebuffer as _)
                .check().unwrap();

            framebuffer
        }).collect();

        FramebufferChain {
            render_pass,
            attachments,
            framebuffers,
        }
    }

    pub fn len(&self) -> usize {
        self.framebuffers.len()
    }
}

#[cfg(test)]
mod tests {
    use vk::traits::*;
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let swapchain = vars.swapchain;

        let attachment_descs = [vk::AttachmentDescription {
            format: swapchain.format,
            samples: vk::SampleCountFlags::_1_BIT,
            load_op: vk::AttachmentLoadOp::DONT_CARE,
            store_op: vk::AttachmentStoreOp::STORE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
            ..Default::default()
        }];
        let subpass_attachment_refs = [vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        }];
        let subpasses = [vk::SubpassDescription {
            pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
            color_attachment_count: subpass_attachment_refs.len() as _,
            p_color_attachments: subpass_attachment_refs.as_ptr(),
            ..Default::default()
        }];
        let create_info = vk::RenderPassCreateInfo {
            attachment_count: attachment_descs.len() as _,
            p_attachments: attachment_descs.as_ptr(),
            subpass_count: subpasses.len() as _,
            p_subpasses: subpasses.as_ptr(),
            ..Default::default()
        };
        let rp = RenderPass::new(Arc::clone(&swapchain.device), &create_info);

        let attachments = AttachmentChain::from_swapchain(&swapchain);
        let framebuffers =
            FramebufferChain::new(Arc::new(rp), vec![Arc::new(attachments)]);

        assert_ne!(framebuffers.len(), 0);
        assert!(!framebuffers.framebuffers.iter().any(|fb| fb.is_null()));
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
