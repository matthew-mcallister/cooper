use std::ptr;
use std::sync::{Arc, Weak};

use derivative::Derivative;
use fnv::FnvHashMap;
use prelude::*;

use crate::*;

#[derive(Debug)]
crate struct RenderPass {
    device: Arc<Device>,
    inner: vk::RenderPass,
    attachments: Vec<vk::AttachmentDescription>,
    subpasses: Vec<SubpassState>,
    dependencies: Vec<vk::SubpassDependency>,
}

#[derive(Debug)]
struct SubpassState {
    input_attchs: Vec<vk::AttachmentReference>,
    color_attchs: Vec<vk::AttachmentReference>,
    resolve_attchs: Vec<vk::AttachmentReference>,
    preserve_attchs: Vec<u32>,
    depth_stencil_attch: Option<vk::AttachmentReference>,
    samples: vk::SampleCountFlags,
}

#[derive(Clone, Debug, Derivative)]
#[derivative(Hash, PartialEq)]
crate struct Subpass {
    #[derivative(Hash(hash_with = "ptr_hash"))]
    #[derivative(PartialEq(compare_with = "ptr_eq"))]
    pass: Arc<RenderPass>,
    index: usize,
}
impl Eq for Subpass {}

#[derive(Debug, Default)]
crate struct SubpassDesc {
    // TODO: Name subpasses?
    crate layouts: Vec<vk::ImageLayout>,
    crate input_attchs: Vec<u32>,
    crate color_attchs: Vec<u32>,
    crate resolve_attchs: Vec<u32>,
    crate preserve_attchs: Vec<u32>,
    crate depth_stencil_attch: Option<u32>,
}

impl Drop for RenderPass {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            dt.destroy_render_pass(self.inner, ptr::null());
        }
    }
}

impl RenderPass {
    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    crate fn inner(&self) -> vk::RenderPass {
        self.inner
    }

    crate fn attachments(&self) -> &[vk::AttachmentDescription] {
        &self.attachments
    }

    crate fn dependencies(&self) -> &[vk::SubpassDependency] {
        &self.dependencies
    }

    crate fn subpasses<'a>(self: &'a Arc<Self>) ->
        impl Iterator<Item = Subpass> + ExactSizeIterator + 'a
    {
        (0..self.subpasses.len())
            .map(move |index| Subpass { pass: Arc::clone(self), index })
    }

    crate fn subpass<'a>(self: &'a Arc<Self>, index: usize) -> Subpass {
        assert!(index < self.subpasses.len());
        Subpass { pass: Arc::clone(self), index }
    }
}

impl Subpass {
    crate fn pass(&self) -> &Arc<RenderPass> {
        &self.pass
    }

    fn state(&self) -> &SubpassState {
        &self.pass().subpasses[self.index]
    }

    crate fn index(&self) -> u32 {
        self.index as _
    }

    crate fn samples(&self) -> vk::SampleCountFlags {
        self.state().samples
    }

    crate fn input_attchs(&self) -> &[vk::AttachmentReference] {
        &self.state().input_attchs
    }

    crate fn color_attchs(&self) -> &[vk::AttachmentReference] {
        &self.state().color_attchs
    }

    crate fn resolve_attchs(&self) -> Option<&[vk::AttachmentReference]> {
        let attchs = &self.state().resolve_attchs;
        (!attchs.is_empty()).then_some(attchs)
    }

    crate fn preserve_attchs(&self) -> &[u32] {
        &self.state().preserve_attchs
    }

    crate fn depth_stencil_attch(&self) -> Option<&vk::AttachmentReference> {
        self.state().depth_stencil_attch.as_ref()
    }
}

fn subpass_state(
    attachments: &[vk::AttachmentDescription],
    desc: SubpassDesc,
) -> SubpassState {
    let get = |idx: u32| &attachments[idx as usize];

    // Infer sample count
    let samples = if let Some(&idx) = desc.color_attchs.first() {
        attachments[idx as usize].samples
    } else if let Some(idx) = desc.depth_stencil_attch {
        attachments[idx as usize].samples
    } else {
        vk::SampleCountFlags::_1_BIT
    };

    // Validate sample counts
    for &idx in desc.color_attchs.iter().chain(desc.depth_stencil_attch.iter())
    {
        assert_eq!(get(idx).samples, samples);
    }
    if !desc.resolve_attchs.is_empty() {
        assert_eq!(desc.color_attchs.len(), desc.resolve_attchs.len());
        for &idx in desc.resolve_attchs.iter() {
            assert_eq!(get(idx).samples, vk::SampleCountFlags::_1_BIT);
        }
    }

    let layouts = desc.layouts;
    let to_ref = |idx| {
        assert!((idx == vk::ATTACHMENT_UNUSED)
            | ((idx as usize) < attachments.len()));
        vk::AttachmentReference {
            attachment: idx,
            layout: layouts[idx as usize],
        }
    };
    let to_refs = |attchs: Vec<_>| attchs.into_iter()
        .map(to_ref).collect(): Vec<_>;
    SubpassState {
        input_attchs: to_refs(desc.input_attchs),
        color_attchs: to_refs(desc.color_attchs),
        resolve_attchs: to_refs(desc.resolve_attchs),
        preserve_attchs: desc.preserve_attchs,
        depth_stencil_attch: desc.depth_stencil_attch.map(to_ref),
        samples,
    }
}

crate unsafe fn create_render_pass(
    device: Arc<Device>,
    attachments: Vec<vk::AttachmentDescription>,
    subpasses: Vec<SubpassDesc>,
    dependencies: Vec<vk::SubpassDependency>,
) -> Arc<RenderPass> {
    let dt = &*device.table;

    let subpasses: Vec<_> = subpasses.into_iter()
        .map(|desc| subpass_state(&attachments, desc)).collect();
    let vk_subpasses: Vec<_> = subpasses.iter().map(|subpass| {
        vk::SubpassDescription {
            pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
            input_attachment_count: subpass.input_attchs.len() as _,
            p_input_attachments: subpass.input_attchs.as_ptr(),
            color_attachment_count: subpass.color_attchs.len() as _,
            p_color_attachments: subpass.color_attchs.as_ptr(),
            p_resolve_attachments: subpass.resolve_attchs.c_ptr(),
            preserve_attachment_count: subpass.preserve_attchs.len() as _,
            p_preserve_attachments: subpass.preserve_attchs.as_ptr(),
            p_depth_stencil_attachment:
                subpass.depth_stencil_attch.as_ref().as_ptr(),
            ..Default::default()
        }
    }).collect();

    let create_info = vk::RenderPassCreateInfo {
        attachment_count: attachments.len() as _,
        p_attachments: attachments.as_ptr(),
        subpass_count: vk_subpasses.len() as _,
        p_subpasses: vk_subpasses.as_ptr(),
        dependency_count: dependencies.len() as _,
        p_dependencies: dependencies.as_ptr(),
        ..Default::default()
    };
    let mut render_pass = vk::null();
    dt.create_render_pass(&create_info, ptr::null(), &mut render_pass)
        .check().unwrap();

    Arc::new(RenderPass {
        device,
        inner: render_pass,
        attachments,
        subpasses,
        dependencies,
    })
}

#[derive(Debug)]
crate struct ScreenPass {
    crate pass: Arc<RenderPass>,
    crate color: Subpass,
}

impl ScreenPass {
    crate fn new(device: Arc<Device>) -> Self {
        unsafe { create_screen_pass(device) }
    }
}

unsafe fn create_screen_pass(device: Arc<Device>) -> ScreenPass {
    let pass = create_render_pass(
        device,
        vec![
            vk::AttachmentDescription {
                format: vk::Format::B8G8R8A8_SRGB,
                samples: vk::SampleCountFlags::_1_BIT,
                load_op: vk::AttachmentLoadOp::DONT_CARE,
                store_op: vk::AttachmentStoreOp::STORE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                ..Default::default()
            },
        ],
        vec![
            SubpassDesc {
                layouts: vec![vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL],
                color_attchs: vec![0],
                ..Default::default()
            },
        ],
        vec![],
    );

    let mut subpasses = pass.subpasses();
    ScreenPass {
        pass: Arc::clone(&pass),
        color: subpasses.next().unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let device = Arc::clone(&vars.swapchain.device);
        let _screen_pass = ScreenPass::new(device);
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
