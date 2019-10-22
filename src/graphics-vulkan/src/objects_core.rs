//! Contains most of the central definitions that power the core of the
//! renderer.
use std::ffi::CString;
use std::ptr;
use std::sync::Arc;

use ccore::name::*;
use fnv::FnvHashMap;
use owning_ref::OwningRef;
use parking_lot::RwLock;

use crate::*;

#[derive(Debug)]
pub struct CoreData {
    config: Config,
    device: Arc<Device>,
    set_layouts: FnvHashMap<Name, DescriptorSetLayout>,
    descriptors: DescriptorAllocator,
    pipe_layouts: FnvHashMap<Name, PipelineLayout>,
    shaders: FnvHashMap<Name, Shader>,
    passes: FnvHashMap<Name, RenderPass>,
    pipelines: RwLock<FnvHashMap<PipelineDesc, GraphicsPipeline>>,
}

impl CoreData {
    crate fn new(device: Arc<Device>) -> Self {
        let mut core = CoreData {
            config: Config {
                width: 1280,
                height: 720,
            },
            descriptors: DescriptorAllocator::new(Arc::clone(&device)),
            set_layouts: Default::default(),
            pipe_layouts: Default::default(),
            shaders: Default::default(),
            passes: Default::default(),
            pipelines: RwLock::new(Default::default()),
            device,
        };
        core.init();
        core
    }

    crate fn init(&mut self) {
        unsafe {
            create_set_layouts(self);
            create_pipe_layouts(self);
            create_render_passes(self);
            create_shaders(self);
        }
    }
}

impl Drop for CoreData {
    fn drop(&mut self) {
        let dt = Arc::clone(&self.device().table);
        unsafe {
            for layout in self.set_layouts.values() {
                dt.destroy_descriptor_set_layout(layout.inner(), ptr::null());
            }
            for layout in self.pipe_layouts.values() {
                dt.destroy_pipeline_layout(layout.inner(), ptr::null());
            }
            for shader in self.shaders.values() {
                dt.destroy_shader_module(shader.inner(), ptr::null());
            }
            for pass in self.passes.values() {
                dt.destroy_render_pass(pass.inner(), ptr::null());
            }
            for pipeline in self.pipelines.get_mut().values() {
                dt.destroy_pipeline(pipeline.inner(), ptr::null());
            }
        }
    }
}

impl CoreData {
    crate fn config(&self) -> &Config {
        &self.config
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    crate fn get_set_layout(&self, name: Name) -> &DescriptorSetLayout {
        &self.set_layouts[&name]
    }

    crate fn alloc_desc_set(&self, set_layout: Name) -> DescriptorSet {
        unsafe {
            if let Some(set) = self.descriptors.allocate(set_layout) {
                return set;
            }

            let layout = self.get_set_layout(set_layout);
            self.descriptors.insert_alloc(set_layout.to_owned(), layout)
        }
    }

    crate fn free_desc_set(&self, set: DescriptorSet) {
        self.descriptors.free(set)
    }

    crate fn get_pipe_layout(&self, name: Name) -> &PipelineLayout {
        &self.pipe_layouts[&name]
    }

    crate fn get_shader(&self, name: Name) -> &Shader {
        &self.shaders[&name]
    }

    crate fn get_pass(&self, name: Name) -> &RenderPass {
        &self.passes[&name]
    }

    crate fn get_pipeline(&self, desc: &PipelineDesc) ->
        impl std::ops::Deref<Target = GraphicsPipeline> + '_
    {
        // TODO: Could parallelize pipeline creation, probably using
        // monitor pattern.
        let pipelines = &self.pipelines;

        // Try to fetch an existing pipeline
        let res = OwningRef::new(pipelines.read())
            .try_map(|pipes| pipes.get(&desc).ok_or(()));
        if let Ok(res) = res { return res; }

        // Not found; create the missing pipeline
        {
            let mut pipelines = pipelines.write();
            let pipe = unsafe { create_graphics_pipeline(self, desc) };
            pipelines.insert(desc.clone(), pipe);
        }

        OwningRef::new(pipelines.read())
            .map(|pipes| &pipes[desc])
    }
}

unsafe fn create_set_layouts(core: &mut CoreData) {
    let device = Arc::clone(core.device());
    let layouts = &mut core.set_layouts;

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
    layouts.insert(Name::new("scene_globals"), layout);

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
    layouts.insert(Name::new("material"), layout);
}

unsafe fn create_pipe_layouts(core: &mut CoreData) {
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

unsafe fn create_shaders(core: &mut CoreData) {
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

unsafe fn create_render_passes(core: &mut CoreData) {
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

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
crate struct PipelineDesc {
}

unsafe fn create_graphics_pipeline(
    core: &CoreData,
    _desc: &PipelineDesc,
) -> GraphicsPipeline {
    let dt = &*core.device().table;

    let vert = core.get_shader(Name::new("triangle_vert"));
    let frag = core.get_shader(Name::new("triangle_frag"));

    let vert_stage = vk::PipelineShaderStageCreateInfo {
        stage: vk::ShaderStageFlags::VERTEX_BIT,
        module: vert.inner(),
        p_name: vert.entry().as_ptr(),
        ..Default::default()
    };
    let frag_stage = vk::PipelineShaderStageCreateInfo {
        stage: vk::ShaderStageFlags::FRAGMENT_BIT,
        module: frag.inner(),
        p_name: frag.entry().as_ptr(),
        ..Default::default()
    };
    let stages = vec![vert_stage, frag_stage];

    let layout_id = Name::new("std_material");
    let layout = core.get_pipe_layout(layout_id).inner();

    let render_pass_id = Name::new("forward");
    let render_pass = core.get_pass(render_pass_id);
    let subpass_id = Name::new("lighting");
    let subpass = render_pass.get_subpass(subpass_id);
    let render_pass = render_pass.inner();

    let vertex_input_state = Default::default();
    let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo {
        topology: vk::PrimitiveTopology::TRIANGLE_LIST,
        ..Default::default()
    };

    let config = core.config();
    let viewports = [config.viewport()];
    let scissors = [config.view_rect()];
    let viewport_state = vk::PipelineViewportStateCreateInfo {
        viewport_count: viewports.len() as _,
        p_viewports: viewports.as_ptr(),
        scissor_count: scissors.len() as _,
        p_scissors: scissors.as_ptr(),
        ..Default::default()
    };

    let rasterization_state = vk::PipelineRasterizationStateCreateInfo {
        cull_mode: vk::CullModeFlags::BACK_BIT,
        line_width: 1.0,
        ..Default::default()
    };

    let multisample_state = vk::PipelineMultisampleStateCreateInfo {
        rasterization_samples: vk::SampleCountFlags::_1_BIT,
        ..Default::default()
    };

    let color_blend_atts = [vk::PipelineColorBlendAttachmentState {
        color_write_mask: vk::ColorComponentFlags::R_BIT
            | vk::ColorComponentFlags::G_BIT
            | vk::ColorComponentFlags::B_BIT
            | vk::ColorComponentFlags::A_BIT,
        ..Default::default()
    }];
    let color_blend_state = vk::PipelineColorBlendStateCreateInfo {
        attachment_count: color_blend_atts.len() as _,
        p_attachments: color_blend_atts.as_ptr(),
        ..Default::default()
    };

    let create_info = vk::GraphicsPipelineCreateInfo {
        stage_count: stages.len() as _,
        p_stages: stages.as_ptr(),
        p_vertex_input_state: &vertex_input_state,
        p_input_assembly_state: &input_assembly_state,
        p_viewport_state: &viewport_state,
        p_rasterization_state: &rasterization_state,
        p_multisample_state: &multisample_state,
        p_color_blend_state: &color_blend_state,
        layout,
        render_pass,
        subpass,
        ..Default::default()
    };
    let create_infos = std::slice::from_ref(&create_info);

    let mut pipelines = [vk::null()];
    dt.create_graphics_pipelines(
        vk::null(),                 // pipelineCache
        create_infos.len() as _,    // createInfoCount
        create_infos.as_ptr(),      // pCreateInfos
        ptr::null(),                // pAllocator
        pipelines.as_mut_ptr(),     // pPipelines
    ).check().unwrap();
    let [inner] = pipelines;

    GraphicsPipeline {
        inner,
        layout: layout_id.to_owned(),
        pass: render_pass_id.to_owned(),
        subpass: subpass_id.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn smoke_test(vars: crate::testing::TestVars) {
        CoreData::new(Arc::clone(&vars.device()));
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
