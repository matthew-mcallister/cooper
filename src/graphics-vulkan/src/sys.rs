//! This module defines the top-level structure of the renderer.
use std::ptr;

use enum_map::Enum
use fnv::FnvHashMap;
use map_lit::*;

use crate::*;

/// The top-level interface to the renderer.
// TODO: Break up into sub-objects?
#[derive(Debug)]
pub struct System {
    // These shouldn't change after initialization.
    crate dt: Arc<vkl::DeviceTable>,
    crate device: Arc<Device>,
    crate shaders: FnvHashMap<&'static str, ShaderObj>,
    crate set_layouts: FnvHashMap<&'static str, SetLayoutObj>,
    crate pipe_layouts: FnvHashMap<&'static str, PipelineLayoutObj>,
    crate render_passes: EnumMap<RenderPassName, RenderPassObj>,
    // These need to be rebuilt if the swapchain is altered.
    crate swapchain: Swapchain,
    crate cur_frame: usize,
    crate frames: Vec<EnumMap<RenderPassName, vk::FrameBuffer>>,
    crate pipelines: PipelineMap,
    // These are directly manipulated by the library consumer
    crate samplers: SamplerStorage,
    crate resources: ResourceStorage,
    crate param_sets: ParameterStorage,
}

impl Drop for System {
    fn drop(&mut self) {
        for frame in self.frames.iter() {
            for &framebuffer in frame.values() {
                self.dt.destroy_framebuffer(framebuffer, ptr::null());
            }
        }
        for obj in self.render_passes.values()
            { self.dt.destroy_render_pass(obj.obj, ptr::null()); }
        for obj in self.pipe_layouts.values()
            { self.dt.destroy_pipeline_layout(obj.obj, ptr::null()); }
        for obj in self.set_layouts.values()
            { self.dt.destroy_descriptor_set_layout(obj.obj, ptr::null()); }
        for obj in self.shaders.values()
            { self.dt.destroy_shader_module(obj.obj, ptr::null()); }
    }
}

#[derive(Clone, Copy, Debug, Enum, Eq, Ord, Hash, PartialEq, PartialOrd)]
crate enum RenderPassName {
    Forward,
}

#[derive(Debug)]
crate struct SetLayoutObj {
    crate obj: vk::DescriptorSetLayout,
    crate counts: DescriptorCounts,
}

#[derive(Debug)]
crate struct ShaderObj {
    crate obj: vk::ShaderModule,
    crate stage: vk::ShaderStageFlags,
    /// Used for debugging
    crate set_layouts: Vec<(u32, BlockName)>,
}

#[derive(Debug)]
crate struct PipelineLayoutObj {
    crate obj: vk::PipelineLayout,
    /// Used for debugging
    crate set_layouts: Vec<BlockName>,
}

#[derive(Debug)]
crate struct RenderPassObj {
    crate obj: vk::RenderPass,
    crate subpasses: FnvHashMap<String, u32>,
}

impl System {
    crate fn cur_frame(&self) -> &EnumMap<RenderPassName, vk::FrameBuffer> {
        self.frames[self.cur_frame]
    }
}

macro_rules! create {
    ($dt:expr, $create:ident, $info:expr) => {
        unsafe {
            let mut obj = vk::null();
            $dt.$create(&$info as _, ptr::null(), &mut obj as _)
                .check().and(Ok(obj))
        }
    }
}

crate fn create_set_layouts(dt: &DeviceTable) ->
    HashMap<&'static str, SetLayoutObj>
{
    let mut collection = HashMap::new();

    macro_rules! create_info {
        ($bindings:expr) => {{
            assert!($bindings.len() > 0);
            vk::DescriptorSetLayoutCreateInfo {
                binding_count: $bindings.len() as _,
                p_bindings: $bindings.as_ptr(),
                ..Default::default()
            }
        }}
    }

    let bindings = [
        vk::DescriptorSetLayoutBinding {
            binding: 0,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            // albedo + metal/roughness + normal
            descriptor_count: 3,
            stage_flags: vk::ShaderStageFlags::FRAGMENT_BIT,
        },
    ];
    let info = create_info!(bindings);
    let obj = create!(dt, create_descriptor_set_layout, info).unwrap();
    let counts = DescriptorCounts::from_bindings(&bindings);
    collection.add("pbr_material", SetLayoutObj { obj, counts });

    let bindings = [
        vk::DescriptorSetLayoutBinding {
            binding: 0,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::VERTEX_BIT |
                vk::ShaderStageFlags::FRAGMENT_BIT,
            ..Default::default()
        },
    ];
    let info = create_info!(bindings);
    let obj = create!(dt, create_descriptor_set_layout, info).unwrap();
    let counts = DescriptorCounts::from_bindings(&bindings);
    collection.insert("object_data", SetLayoutObj { obj, counts });

    collection
}

crate fn create_render_passes(dt: &DeviceTable) ->
    HashMap<&'static str, RenderPassObj>
{
    let mut collection = HashMap::new(Arc::clone(dt));

    let attachments = &[
        vk::AttachmentDescription {
            format: vk::Format::B8G8R8A8_SRGB,
            samples: vk::SampleCountFlags::_1_BIT,
            load_op: vk::AttachmentLoadOp::CLEAR,
            store_op: vk::AttachmentStoreOp::STORE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
            ..Default::default()
        },
        vk::AttachmentDescription {
            format: vk::Format::D32_SFLOAT,
            samples: vk::SampleCountFlags::_1_BIT,
            load_op: vk::AttachmentLoadOp::CLEAR,
            store_op: vk::AttachmentStoreOp::DONT_CARE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            ..Default::default()
        },
    ];

    let depth_att = vk::AttachmentReference {
        attachment: 1,
        layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    };
    let color_att = vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    };
    let subpasses = [
        // depth
        vk::SubpassDescription {
            p_depth_stencil_attachment: &depth_att as _,
            ..Default::default()
        },
        // lighting
        vk::SubpassDescription {
            p_color_attachments: &color_att as _,
            color_attachment_count: 1,
            p_depth_stencil_attachment: &depth_att as _,
            ..Default::default()
        },
        // TODO: lighting-only pass?
    ];
    let subpass_map = hashmap! {
        "depth" => 0,
        "lighting" => 1,
    };

    let dependencies = [
        // Depth test depends on depth writes finishing
        vk::SubpassDependency {
            src_subpass: 0, // depth
            dst_subpass: 1, // lighting
            src_stage_mask: vk::PipelineStageFlags::LATE_FRAGMENT_TESTS_BIT,
            dst_stage_mask: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS_BIT,
            src_access_mask:
                vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE_BIT,
            dst_access_mask:
                vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ_BIT
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE_BIT,
            ..Default::default()
        },
    ];

    let info = vk::RenderPassCreateInfo {
        attachment_count: attachments.len() as _,
        p_attachments: attachments.as_ptr(),
        subpass_count: subpasses.len() as _,
        p_subpasses: subpasses.as_ptr(),
        dependency_count: dependencies.len() as _,
        p_dependencies: dependencies.as_ptr(),
        ..Default::default()
    };
    let obj = create!(dt, create_render_pass, info).unwrap();
    collection.insert(
        RenderPassName::Forward,
        RenderPassObj { obj, subpasses: subpass_map },
    );

    collection
}

crate fn create_shaders(dt: &DeviceTable) -> HashMap<&'static str, ShaderObj> {
    let mut collection = HashMap::new(Arc::clone(dt));

    macro_rules! shader_source {
        ($name:expr) => {
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/data/spirv/",
                $name,
                ".spv",
            ))
        }
    }

    macro_rules! shader {
        ($dt:expr, $name:expr, $stage:ident, $set_layouts:tt) => {
            let code = shader_source!($name);
            assert_eq!(code.len() % 4, 0);
            let info = vk::ShaderModuleCreateInfo {
                code_size: code.len() as _,
                p_code: code.as_ptr() as _,
                ..Default::default()
            };
            let obj = create!($dt, create_shader_module, info).unwrap();
            collection.insert($name, ShaderObj {
                obj,
                stage: vk::ShaderStageFlags::$stage,
                set_layouts: vec! $set_layouts,
            });
        }
    }

    shader!(dt, "depth_vert", VERTEX_BIT, [(1, "object_data")]);
    shader!(dt, "static_pbr_vert", VERTEX_BIT, [(1, "object_data")]);
    shader!(dt, "pbr_frag", FRAGMENT_BIT, [(0, "pbr_material")]);

    collection
}

crate fn create_pipeline_layouts(
    dt: &DeviceTable,
    set_layouts: &HashMap<String, SetLayoutObj>,
) -> HashMap<&'static str, PipelineLayoutObj> {
    let mut collection = HashMap::new(Arc::clone(dt));

    let defs: &[(&'static str, &[&'static str])] = &[
        ("depth", &["object_data"]),
        ("pbr", &["pbr_material", "object_data"]),
    ];
    for &(name, layout_names) in defs {
        let layouts: Vec<_> =
            layout_names.iter().map(|&s| set_layouts.get(s).obj).collect();
        let create_info = vk::PipelineLayoutCreateInfo {
            set_layout_count: layouts.len() as _,
            p_set_layouts: layouts.as_ptr(),
            ..Default::default()
        };
        let mut obj = vk::null();
        unsafe {
            dt.create_pipeline_layout
                (&create_info as _, ptr::null(), &mut obj as _)
                .check().unwrap();
        }
        collection.insert(name, PipelineLayoutObj {
            obj,
            set_layouts: layout_names.to_owned(),
        });
    }

    collection
}
