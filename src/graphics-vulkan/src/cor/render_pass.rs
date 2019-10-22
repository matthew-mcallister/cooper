use std::ptr;

use ccore::name::*;
use fnv::FnvHashMap;

use crate::*;

#[derive(Debug)]
pub struct RenderPass {
    inner: vk::RenderPass,
    attachments: Vec<vk::AttachmentDescription>,
    subpasses: FnvHashMap<Name, u32>,
}

impl RenderPass {
    pub fn inner(&self) -> vk::RenderPass {
        self.inner
    }

    pub fn attachments(&self) -> &[vk::AttachmentDescription] {
        &self.attachments
    }

    pub fn get_subpass(&self, key: Name) -> u32 {
        self.subpasses[&key]
    }
}

pub unsafe fn create_render_pass(
    device: &Device,
    create_info: &vk::RenderPassCreateInfo,
    subpass_names: Vec<Name>,
) -> RenderPass {
    let dt = &*device.table;

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

    RenderPass {
        inner: render_pass,
        attachments,
        subpasses,
    }
}
