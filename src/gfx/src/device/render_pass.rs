use std::ptr;
use std::sync::Arc;

use derivative::Derivative;
use enum_map::Enum;
use prelude::*;

use crate::*;

#[derive(Debug)]
crate struct RenderPass {
    device: Arc<Device>,
    inner: vk::RenderPass,
    attachments: Vec<AttachmentDescription>,
    subpasses: Vec<SubpassState>,
    dependencies: Vec<vk::SubpassDependency>,
}

#[derive(Clone, Copy, Debug, Enum, Eq, Hash, PartialEq)]
crate enum Attachment {
    /// SRGB screen buffer
    Backbuffer,
    DepthStencil,
    /// HDR light buffer
    Hdr,
    Normal,
    Albedo,
}

#[derive(Clone, Copy, Debug, Derivative)]
#[derivative(Default)]
crate struct AttachmentDescription {
    // TODO: It's unfortunate that this has a default value. Maybe
    // default() should just panic?
    #[derivative(Default(value = "Attachment::Backbuffer"))]
    crate name: Attachment,
    #[derivative(Default(value = "Format::R8"))]
    crate format: Format,
    crate samples: SampleCount,
    // These fields follow a possibly dumb but reasonable-sounding
    // convention: if you don't specify it, you don't care about it.
    #[derivative(Default(value = "vk::AttachmentLoadOp::DONT_CARE"))]
    crate load_op: vk::AttachmentLoadOp,
    #[derivative(Default(value = "vk::AttachmentStoreOp::DONT_CARE"))]
    crate store_op: vk::AttachmentStoreOp,
    #[derivative(Default(value = "vk::AttachmentLoadOp::DONT_CARE"))]
    crate stencil_load_op: vk::AttachmentLoadOp,
    #[derivative(Default(value = "vk::AttachmentStoreOp::DONT_CARE"))]
    crate stencil_store_op: vk::AttachmentStoreOp,
    crate initial_layout: vk::ImageLayout,
    crate final_layout: vk::ImageLayout,
}

#[derive(Debug)]
struct SubpassState {
    input_attchs: Vec<vk::AttachmentReference>,
    color_attchs: Vec<vk::AttachmentReference>,
    resolve_attchs: Vec<vk::AttachmentReference>,
    preserve_attchs: Vec<u32>,
    depth_stencil_attch: Option<vk::AttachmentReference>,
    samples: SampleCount,
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
    crate unsafe fn new(
        device: Arc<Device>,
        attachments: Vec<AttachmentDescription>,
        subpasses: Vec<SubpassDesc>,
        dependencies: Vec<vk::SubpassDependency>,
    ) -> Arc<Self> {
        create_render_pass(device, attachments, subpasses, dependencies)
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    crate fn inner(&self) -> vk::RenderPass {
        self.inner
    }

    crate fn attachments(&self) -> &[AttachmentDescription] {
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

    crate fn is_input_attachment(&self, index: usize) -> bool {
        assert!(index < self.attachments.len());
        self.subpasses.iter()
            .flat_map(|subpass| subpass.input_attchs.iter())
            .any(|aref| aref.attachment == index as u32)
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

    crate fn samples(&self) -> SampleCount {
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

impl From<AttachmentDescription> for vk::AttachmentDescription {
    fn from(desc: AttachmentDescription) -> Self {
        Self {
            format: desc.format.into(),
            samples: desc.samples.into(),
            load_op: desc.load_op,
            store_op: desc.store_op,
            stencil_load_op: desc.stencil_load_op,
            stencil_store_op: desc.stencil_store_op,
            initial_layout: desc.initial_layout,
            final_layout: desc.final_layout,
            ..Default::default()
        }
    }
}

crate fn input_attachment_layout(format: Format) -> vk::ImageLayout {
    if format.is_depth_stencil() {
        vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL
    } else {
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
    }
}

fn subpass_samples(
    attachments: &[AttachmentDescription],
    desc: &SubpassDesc,
) -> SampleCount {
    let idx = desc.color_attchs.first()
        .or(desc.input_attchs.first())
        .or(desc.depth_stencil_attch.as_ref());
    try_opt! { return attachments[*idx? as usize].samples; };
    SampleCount::One
}

// TODO: Layouts should be defined manually. If necessary, a helper
// function can fill in the appropriate layouts.
fn subpass_state(attachments: &[AttachmentDescription], desc: SubpassDesc) ->
    SubpassState
{
    let get = |idx: u32| &attachments[idx as usize];

    validate_subpass(attachments, &desc);

    let samples = subpass_samples(attachments, &desc);

    let input_attchs: Vec<_> = desc.input_attchs.iter().map(|&idx| {
        let layout = input_attachment_layout(get(idx).format);
        vk::AttachmentReference { attachment: idx, layout }
    }).collect();
    let color_attchs: Vec<_> = desc.color_attchs.iter().map(|&idx| {
        let layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
        vk::AttachmentReference { attachment: idx, layout }
    }).collect();
    let depth_stencil_attch = try_opt!(vk::AttachmentReference {
        attachment: desc.depth_stencil_attch?,
        // TODO: DEPTH_STENCIL_READ_ONLY_OPTIMAL may be useful here
        layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    });
    let resolve_attchs = desc.resolve_attchs.iter().map(|&idx| {
        vk::AttachmentReference {
            attachment: idx,
            layout: vk::ImageLayout::UNDEFINED,
        }
    }).collect();

    SubpassState {
        input_attchs,
        color_attchs,
        resolve_attchs,
        preserve_attchs: desc.preserve_attchs,
        depth_stencil_attch,
        samples,
    }
}

fn validate_subpass(attachments: &[AttachmentDescription], desc: &SubpassDesc)
{
    let get = |idx: u32| &attachments[idx as usize];

    // Sample count
    let samples = subpass_samples(attachments, desc);
    for &idx in desc.color_attchs.iter()
        .chain(desc.input_attchs.iter())
        .chain(desc.depth_stencil_attch.iter())
    {
        assert_eq!(get(idx).samples, samples);
    }

    // Disallow unused references except as resolve attachments
    for &idx in desc.color_attchs.iter()
        .chain(desc.input_attchs.iter())
        .chain(desc.preserve_attchs.iter())
        .chain(desc.depth_stencil_attch.iter())
    {
        assert_ne!(idx, vk::ATTACHMENT_UNUSED);
    }

    // Disallow multiple attachment use. This is sometimes allowed
    // (e.g. input feedback) but not really desired.
    let mut counts = vec![0u32; attachments.len()];
    for &idx in desc.color_attchs.iter()
        .chain(desc.input_attchs.iter())
        .chain(desc.preserve_attchs.iter())
        .chain(desc.depth_stencil_attch.iter())
        .chain(desc.resolve_attchs.iter())
        .filter(|&&idx| idx != vk::ATTACHMENT_UNUSED)
    {
        counts[idx as usize] += 1;
    }
    for (i, count) in counts.into_iter().enumerate() {
        assert!(count <= 1, "[{}] = {}", i, count);
    }

    // Validate sample counts
    let samples = subpass_samples(attachments, desc);
    for &idx in desc.color_attchs.iter()
        .chain(desc.input_attchs.iter())
        .chain(desc.depth_stencil_attch.iter())
    {
        assert_eq!(get(idx).samples, samples);
    }

    // Resolve attachments
    if !desc.resolve_attchs.is_empty() {
        assert_eq!(desc.color_attchs.len(), desc.resolve_attchs.len());
        for (src, &dst) in desc.resolve_attchs.iter().enumerate() {
            assert_eq!(get(dst).samples, SampleCount::One);
            assert_eq!(get(src as _).format, get(dst).format)
        }
    }

    // Formats
    for &idx in desc.color_attchs.iter() {
        assert!(!get(idx).format.is_depth_stencil());
    }
    if let Some(idx) = desc.depth_stencil_attch {
        assert!(get(idx).format.is_depth_stencil());
    }
}

fn validate_dependencies(
    subpasses: &[SubpassState],
    dependencies: &[vk::SubpassDependency],
) {
    for dep in dependencies.iter() {
        assert!((dep.src_subpass == vk::SUBPASS_EXTERNAL)
            | (dep.src_subpass < subpasses.len() as _));
        assert!((dep.dst_subpass == vk::SUBPASS_EXTERNAL)
            | (dep.dst_subpass < subpasses.len() as _));
    }
}

unsafe fn create_render_pass(
    device: Arc<Device>,
    attachments: Vec<AttachmentDescription>,
    // TODO:
    //attachments: EnumMap<Attachment, AttachmentDescription>,
    //bindings: Vec<Attachment>, // no dupes
    subpasses: Vec<SubpassDesc>,
    dependencies: Vec<vk::SubpassDependency>,
) -> Arc<RenderPass> {
    let dt = &*device.table;

    let vk_attachments: Vec<vk::AttachmentDescription> = attachments.iter()
        .map(|&attch| attch.into())
        .collect();

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

    validate_dependencies(&subpasses, &dependencies);

    let create_info = vk::RenderPassCreateInfo {
        attachment_count: vk_attachments.len() as _,
        p_attachments: vk_attachments.as_ptr(),
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

// Simplified render pass with G-buffer.
#[cfg(test)]
crate unsafe fn create_test_pass(device: Arc<Device>) -> Arc<RenderPass> {
    use vk::AccessFlags as Af;
    use vk::PipelineStageFlags as Pf;

    // Defining render passes is rather technical and so is done
    // manually rather than via a half-baked algorithm.
    RenderPass::new(
        device,
        vec![
            // Screen
            AttachmentDescription {
                name: Attachment::Backbuffer,
                format: Format::BGRA8_SRGB,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                ..Default::default()
            },
            // HDR lighting buffer
            // TODO: Not sure if it's a better practice to set
            // initial_layout or not.
            AttachmentDescription {
                name: Attachment::Hdr,
                format: Format::RGBA16F,
                final_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                ..Default::default()
            },
            // Depth/stencil
            AttachmentDescription {
                name: Attachment::DepthStencil,
                format: Format::D32F_S8,
                load_op: vk::AttachmentLoadOp::CLEAR,
                final_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                ..Default::default()
            },
            // Normals
            AttachmentDescription {
                name: Attachment::Normal,
                format: Format::RGBA8,
                final_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                ..Default::default()
            },
            // Albedo
            AttachmentDescription {
                name: Attachment::Albedo,
                format: Format::RGBA8,
                final_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                ..Default::default()
            },
        ],
        vec![
            // G-buffer pass
            SubpassDesc {
                color_attchs: vec![3, 4],
                depth_stencil_attch: Some(2),
                ..Default::default()
            },
            // Lighting pass
            SubpassDesc {
                color_attchs: vec![1],
                input_attchs: vec![2, 3, 4],
                ..Default::default()
            },
            // Tonemapping
            SubpassDesc {
                color_attchs: vec![0],
                input_attchs: vec![1],
                ..Default::default()
            },
        ],
        vec![
            // Image layout transition barrier; see Vulkan
            // synchronization examples webpage
            vk::SubpassDependency {
                src_subpass: vk::SUBPASS_EXTERNAL,
                dst_subpass: 2,
                src_stage_mask: Pf::COLOR_ATTACHMENT_OUTPUT_BIT,
                dst_stage_mask: Pf::COLOR_ATTACHMENT_OUTPUT_BIT,
                src_access_mask: Default::default(),
                dst_access_mask: Af::COLOR_ATTACHMENT_WRITE_BIT,
                ..Default::default()
            },
            vk::SubpassDependency {
                src_subpass: 0,
                dst_subpass: 1,
                src_stage_mask: Pf::EARLY_FRAGMENT_TESTS_BIT
                    | Pf::LATE_FRAGMENT_TESTS_BIT
                    | Pf::COLOR_ATTACHMENT_OUTPUT_BIT,
                dst_stage_mask: Pf::FRAGMENT_SHADER_BIT,
                src_access_mask: Af::COLOR_ATTACHMENT_WRITE_BIT
                    | Af::DEPTH_STENCIL_ATTACHMENT_WRITE_BIT,
                dst_access_mask: Af::INPUT_ATTACHMENT_READ_BIT,
                dependency_flags: vk::DependencyFlags::BY_REGION_BIT,
            },
            vk::SubpassDependency {
                src_subpass: 1,
                dst_subpass: 2,
                src_stage_mask: Pf::COLOR_ATTACHMENT_OUTPUT_BIT,
                dst_stage_mask: Pf::FRAGMENT_SHADER_BIT,
                src_access_mask: Af::COLOR_ATTACHMENT_WRITE_BIT,
                dst_access_mask: Af::INPUT_ATTACHMENT_READ_BIT,
                dependency_flags: vk::DependencyFlags::BY_REGION_BIT,
            },
            // Post-pass synchronization is implicit.
        ],
    )
}

#[cfg(test)]
mod tests {
    use crate::*;
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let _trivial_pass = TrivialPass::new(Arc::clone(&vars.device()));
    }

    unsafe fn deferred_test(vars: testing::TestVars) {
        let _pass = create_test_pass(Arc::clone(vars.device()));
    }

    unit::declare_tests![
        smoke_test,
        deferred_test,
    ];
}

unit::collect_tests![tests];
