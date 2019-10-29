use std::ffi::CString;
use std::sync::Arc;

use ccore::name::*;

use crate::*;

pub(super) unsafe fn create_set_layouts(core: &mut CoreData) {
    let device = Arc::clone(core.device());

    let policy = DescriptorSetAllocPolicy::default();

    let bindings = [vk::DescriptorSetLayoutBinding {
        binding: 0,
        descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
        descriptor_count: 1,
        stage_flags: vk::ShaderStageFlags::VERTEX_BIT
            | vk::ShaderStageFlags::FRAGMENT_BIT,
        ..Default::default()
    }];
    let create_info = vk::DescriptorSetLayoutCreateInfo {
        binding_count: bindings.len() as _,
        p_bindings: bindings.as_ptr(),
        ..Default::default()
    };
    let layout = create_descriptor_set_layout(&device, &create_info, policy);
    core.insert_set_layout(Name::new("scene_globals"), layout);

    let bindings = [vk::DescriptorSetLayoutBinding {
        binding: 0,
        descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        descriptor_count: 1,
        stage_flags: vk::ShaderStageFlags::FRAGMENT_BIT,
        ..Default::default()
    }];
    let create_info = vk::DescriptorSetLayoutCreateInfo {
        binding_count: bindings.len() as _,
        p_bindings: bindings.as_ptr(),
        ..Default::default()
    };
    let layout = create_descriptor_set_layout(&device, &create_info, policy);
    core.insert_set_layout(Name::new("material"), layout);
}

pub(super) unsafe fn create_pipe_layouts(core: &mut CoreData) {
    core.pipe_layouts.insert(
        Name::new("std_material"),
        create_pipeline_layout(
            &core,
            vec![Name::new("scene_globals"), Name::new("material")],
        ),
    );
}

macro_rules! include_shader {
    ($name:expr) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            concat!("/generated/shaders/", $name),
        ))
    }
}

macro_rules! create_shaders {
    (
        $core:expr,
        [$({
            name: $name:expr,
            bindings: [$(($binding_idx:expr, $binding_name:expr)),*$(,)*]$(,)*
        }),*$(,)*]$(,)*
    ) => {
        $(
            let desc = ShaderDesc {
                entry: CString::new("main".to_owned()).unwrap(),
                code: include_shader!(concat!($name, ".spv")).to_vec(),
                set_bindings: vec![
                    $(($binding_idx, Name::new($binding_name)),)*
                ],
            };
            let shader = create_shader($core.device(), desc);
            $core.shaders.insert(Name::new($name), shader);
        )*
    }
}

pub(super) unsafe fn create_shaders(core: &mut CoreData) {
    create_shaders!(core, [
        {
            name: "triangle_vert",
            bindings: [],
        },
        {
            name: "triangle_frag",
            bindings: [],
        },
        {
            name: "example_vert",
            bindings: [(0, "scene_globals")],
        },
        {
            name: "example_frag",
            bindings: [(0, "scene_globals")],
        },
    ]);
}

pub(super) unsafe fn create_render_passes(core: &mut CoreData) {
    let device = &*core.device;
    let passes = &mut core.passes;

    let attachment_descs = [vk::AttachmentDescription {
        format: vk::Format::B8G8R8A8_SRGB,
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
    let subpass_names = vec![Name::new("lighting")];
    let create_info = vk::RenderPassCreateInfo {
        attachment_count: attachment_descs.len() as _,
        p_attachments: attachment_descs.as_ptr(),
        subpass_count: subpasses.len() as _,
        p_subpasses: subpasses.as_ptr(),
        ..Default::default()
    };
    passes.insert(
        Name::new("forward"),
        create_render_pass(device, &create_info, subpass_names),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn smoke_test(vars: crate::testing::TestVars) {
        let device = Arc::clone(vars.swapchain.device());
        CoreData::new(device, &vars.queues, vars.config);
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
