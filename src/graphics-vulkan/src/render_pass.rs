use std::ptr;
use std::sync::Arc;

use fnv::FnvHashMap;
use prelude::*;

use crate::*;

#[derive(Debug)]
pub struct RenderPass {
    pub inner: vk::RenderPass,
    pub attachments: Vec<vk::AttachmentDescription>,
    pub subpasses: FnvHashMap<String, u32>,
}

#[derive(Debug)]
pub struct RenderPassManager {
    crate device: Arc<Device>,
    render_passes: FnvHashMap<String, RenderPass>,
}

impl Drop for RenderPassManager {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            for render_pass in self.render_passes.values() {
                dt.destroy_render_pass(render_pass.inner, ptr::null());
            }
        }
    }
}

impl RenderPassManager {
    pub fn new(device: Arc<Device>) -> Self {
        RenderPassManager {
            device,
            render_passes: Default::default(),
        }
    }

    pub unsafe fn create_render_pass(
        &mut self,
        name: String,
        create_info: &vk::RenderPassCreateInfo,
        subpass_names: Vec<String>,
    ) {
        let dt = &*self.device.table;

        let attachments = std::slice::from_raw_parts
            (create_info.p_attachments, create_info.attachment_count as _);
        let attachments = attachments.to_vec();

        let mut render_pass = vk::null();
        dt.create_render_pass(create_info, ptr::null(), &mut render_pass)
            .check().unwrap();

        let num_subpasses = subpass_names.len();
        let subpasses: FnvHashMap<_, _> = subpass_names.into_iter()
            .enumerate()
            .map(|(idx, name)| (name, idx as _))
            .collect();
        assert_eq!(subpasses.len(), num_subpasses, "duplicate subpass name");

        let render_pass = RenderPass {
            inner: render_pass,
            attachments,
            subpasses,
        };
        insert_unique!(self.render_passes, name, render_pass);
    }

    pub fn get(&self, key: impl AsRef<str>) -> &RenderPass {
        &self.render_passes[key.as_ref()]
    }

    pub unsafe fn create_framebuffers(
        &self,
        render_pass: String,
        attachments: Vec<Arc<AttachmentChain>>,
    ) -> FramebufferChain {
        FramebufferChain::new(&self, render_pass, attachments)
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

// TODO: Seems like the wrong abstraction; a trait might be better.
#[derive(Debug)]
pub struct FramebufferChain {
    pub device: Arc<Device>,
    pub render_pass: String,
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

impl FramebufferChain {
    pub unsafe fn new(
        render_passes: &RenderPassManager,
        render_pass: String,
        attachments: Vec<Arc<AttachmentChain>>,
    ) -> Self {
        let device = Arc::clone(&render_passes.device);
        let dt = &*device.table;
        let render_pass_id = render_pass;
        let render_pass = render_passes.get(&render_pass_id);

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
            dt.create_framebuffer(&create_info, ptr::null(), &mut framebuffer)
                .check().unwrap();

            framebuffer
        }).collect();

        FramebufferChain {
            device,
            render_pass: render_pass_id,
            extent,
            attachments,
            framebuffers,
        }
    }

    pub fn len(&self) -> usize {
        self.framebuffers.len()
    }

    pub fn rect(&self) -> vk::Rect2D {
        vk::Rect2D::new(vk::Offset2D::new(0, 0), self.extent)
    }
}

#[cfg(test)]
crate unsafe fn create_test_render_passes(vars: &testing::TestVars) ->
    (RenderPassManager, Arc<AttachmentChain>, FramebufferChain)
{
    let swapchain = Arc::clone(&vars.swapchain);

    let mut render_passes =
        RenderPassManager::new(Arc::clone(&swapchain.device));

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
    let subpass_names = vec!["lighting".to_owned()];
    let create_info = vk::RenderPassCreateInfo {
        attachment_count: attachment_descs.len() as _,
        p_attachments: attachment_descs.as_ptr(),
        subpass_count: subpasses.len() as _,
        p_subpasses: subpasses.as_ptr(),
        ..Default::default()
    };
    render_passes
        .create_render_pass("forward".to_owned(), &create_info, subpass_names);

    let attachments = Arc::new(AttachmentChain::from_swapchain(&swapchain));
    let framebufs = render_passes.create_framebuffers(
        "forward".to_owned(),
        vec![Arc::clone(&attachments)],
    );

    (render_passes, attachments, framebufs)
}

#[cfg(test)]
mod tests {
    use vk::traits::*;
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let (_, _, framebuffers) = create_test_render_passes(&vars);

        assert_ne!(framebuffers.len(), 0);
        assert!(!framebuffers.framebuffers.iter().any(|fb| fb.is_null()));
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
