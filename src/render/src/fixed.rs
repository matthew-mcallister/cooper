//! This module defines much of the fixed functionality of the rendering
//! path. This includes descriptor set layouts, shaders, render passes,
//! and more. Each of these objects is named so it can be referred to
//! elsewhere in the engine.

// TODO: Use enum instead of strings for names

use std::collections::HashMap;
use std::ptr;
use std::sync::Arc;

use maplit::hashmap;

use crate::*;

crate type BlockName = &'static str;
crate type BlockNameOwned = String;

crate trait VkObject {
    unsafe fn destroy(self, dt: &vkl::DeviceTable);
}

crate trait Collection {
    type Object;

    fn add(&mut self, key: impl Into<BlockNameOwned>, obj: Self::Object);

    fn get(&self, key: &str) -> &Self::Object;

    unsafe fn destroy(&mut self, key: &str);
}

// TODO: Use FNV hasher
#[derive(Debug)]
crate struct HashCollection<T: VkObject> {
    dt: Arc<vkl::DeviceTable>,
    objects: HashMap<BlockNameOwned, T>,
}

impl<T: VkObject> HashCollection<T> {
    crate fn new(dt: Arc<vkl::DeviceTable>) -> Self {
        HashCollection {
            dt,
            objects: HashMap::new(),
        }
    }
}

impl<T: VkObject> Drop for HashCollection<T> {
    fn drop(&mut self) {
        for (_, obj) in self.objects.drain() {
            unsafe { obj.destroy(&self.dt) }
        }
    }
}

impl<T: VkObject> Collection for HashCollection<T> {
    type Object = T;

    fn add(&mut self, key: impl Into<BlockNameOwned>, obj: Self::Object) {
        insert_nodup!(self.objects, key.into(), obj);
    }

    fn get(&self, key: &str) -> &Self::Object {
        &self.objects[key]
    }

    unsafe fn destroy(&mut self, key: &str) {
        let obj = self.objects.remove(key).unwrap();
        obj.destroy(&self.dt);
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

#[derive(Debug)]
crate struct SamplerObj {
    crate obj: vk::Sampler,
}

impl VkObject for SamplerObj {
    unsafe fn destroy(self, dt: &vkl::DeviceTable) {
        dt.destroy_sampler(self.obj, ptr::null());
    }
}

#[derive(Debug)]
crate struct SetLayoutObj {
    crate obj: vk::DescriptorSetLayout,
    crate counts: DescriptorCounts,
}

impl VkObject for SetLayoutObj {
    unsafe fn destroy(self, dt: &vkl::DeviceTable) {
        dt.destroy_descriptor_set_layout(self.obj, ptr::null());
    }
}

#[derive(Debug)]
crate struct RenderPassObj {
    crate obj: vk::RenderPass,
    crate subpasses: HashMap<String, u32>,
}

impl VkObject for RenderPassObj {
    unsafe fn destroy(self, dt: &vkl::DeviceTable) {
        dt.destroy_render_pass(self.obj, ptr::null());
    }
}

#[derive(Debug)]
crate struct ShaderObj {
    crate obj: vk::ShaderModule,
    crate stage: vk::ShaderStageFlags,
    /// Used for debugging
    crate set_layouts: Vec<(u32, BlockName)>,
}

impl VkObject for ShaderObj {
    unsafe fn destroy(self, dt: &vkl::DeviceTable) {
        dt.destroy_shader_module(self.obj, ptr::null());
    }
}

#[derive(Debug)]
crate struct PipelineLayoutObj {
    crate obj: vk::PipelineLayout,
    /// Used for debugging
    crate set_layouts: Vec<BlockName>,
}

impl VkObject for PipelineLayoutObj {
    unsafe fn destroy(self, dt: &vkl::DeviceTable) {
        dt.destroy_pipeline_layout(self.obj, ptr::null());
    }
}

crate fn stock_samplers(swapchain: &Swapchain) -> HashCollection<SamplerObj> {
    let dt = &swapchain.dt;
    let mut collection = HashCollection::new(Arc::clone(dt));

    let info = vk::SamplerCreateInfo {
        mag_filter: vk::Filter::LINEAR,
        min_filter: vk::Filter::LINEAR,
        mipmap_mode: vk::SamplerMipmapMode::LINEAR,
        address_mode_u: vk::SamplerAddressMode::REPEAT,
        address_mode_v: vk::SamplerAddressMode::REPEAT,
        address_mode_w: vk::SamplerAddressMode::REPEAT,
        // TODO: anisotropic filtering
        ..Default::default()
    };
    let obj = create!(dt, create_sampler, info).unwrap();
    collection.add("std_mip", SamplerObj { obj });

    collection
}

crate fn stock_set_layouts(
    swapchain: &Swapchain,
    samplers: &HashCollection<SamplerObj>,
) -> HashCollection<SetLayoutObj> {
    let dt = &swapchain.dt;
    let mut collection = HashCollection::new(Arc::clone(dt));

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
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::FRAGMENT_BIT,
            ..Default::default()
        },
    ];
    let info = create_info!(bindings);
    let obj = create!(dt, create_descriptor_set_layout, info).unwrap();
    let counts = DescriptorCounts::from_bindings(&bindings);
    collection.add("scene_globals", SetLayoutObj { obj, counts });

    let sampler = samplers.get("std_mip").obj;
    let im_samplers = [sampler; 3];
    let bindings = [
        vk::DescriptorSetLayoutBinding {
            binding: 0,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            // albedo + metal/roughness + normal
            descriptor_count: 3,
            stage_flags: vk::ShaderStageFlags::FRAGMENT_BIT,
            p_immutable_samplers: &im_samplers as _,
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
            stage_flags: vk::ShaderStageFlags::VERTEX_BIT,
            ..Default::default()
        },
    ];
    let info = create_info!(bindings);
    let obj = create!(dt, create_descriptor_set_layout, info).unwrap();
    let counts = DescriptorCounts::from_bindings(&bindings);
    collection.add("object_data", SetLayoutObj { obj, counts });

    collection
}

crate fn stock_render_passes(swapchain: &Swapchain) ->
    HashCollection<RenderPassObj>
{
    let dt = &swapchain.dt;
    let mut collection = HashCollection::new(Arc::clone(dt));

    let attachments = &[
        vk::AttachmentDescription {
            format: swapchain.create_info.image_format,
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
        "depth".to_string() => 0,
        "lighting".to_string() => 1,
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
    collection.add("forward", RenderPassObj { obj, subpasses: subpass_map });

    collection
}

crate fn stock_shaders(swapchain: &Swapchain) -> HashCollection<ShaderObj> {
    let dt = &swapchain.dt;
    let mut collection = HashCollection::new(Arc::clone(dt));

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
            collection.add($name, ShaderObj {
                obj,
                stage: vk::ShaderStageFlags::$stage,
                set_layouts: vec! $set_layouts,
            });
        }
    }

    shader!(dt, "depth_vert", VERTEX_BIT, [(2, "object_data")]);
    shader!(dt, "static_pbr_vert", VERTEX_BIT,
        [(0, "scene_globals"), (2, "object_data")]);
    shader!(dt, "pbr_frag", FRAGMENT_BIT, [(1, "pbr_material")]);

    collection
}

crate fn stock_pipeline_layouts(
    swapchain: &Swapchain,
    set_layouts: &HashCollection<SetLayoutObj>,
) -> HashCollection<PipelineLayoutObj> {
    let dt = &swapchain.dt;
    let mut collection = HashCollection::new(Arc::clone(dt));

    let defs: &[(&'static str, &[&'static str])] = &[
        ("scene_globals", &["scene_globals"]),
        ("pbr", &["scene_globals", "pbr_material", "object_data"]),
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
        collection.add(name, PipelineLayoutObj {
            obj,
            set_layouts: layout_names.to_owned(),
        });
    }

    collection
}
