use std::ffi::c_void;
use std::hash::{Hash, Hasher};
use std::ptr;
use std::sync::Arc;

use derivative::Derivative;
use enum_map::EnumMap;
use fnv::FnvHashMap;
use prelude::*;

use crate::*;

#[derive(Debug)]
crate struct PipelineLayout {
    device: Arc<Device>,
    inner: vk::PipelineLayout,
    set_layouts: Vec<Arc<DescriptorSetLayout>>,
}

#[derive(Debug)]
crate struct GraphicsPipeline {
    device: Arc<Device>,
    inner: vk::Pipeline,
    layout: Arc<PipelineLayout>,
    pass: Arc<RenderPass>,
    subpass: u32,
}

#[derive(Clone, Debug, Derivative)]
#[derivative(Hash, PartialEq)]
crate struct GraphicsPipelineDesc {
    crate subpass: Subpass,
    #[derivative(Hash(hash_with = "ptr_hash"))]
    #[derivative(PartialEq(compare_with = "ptr_eq"))]
    crate layout: Arc<PipelineLayout>,
    #[derivative(Hash(hash_with = "ptr_hash"))]
    #[derivative(PartialEq(compare_with = "ptr_eq"))]
    crate vertex_layout: Arc<VertexLayout>,
    #[derivative(Hash(hash_with = "byte_hash"))]
    #[derivative(PartialEq(compare_with = "byte_eq"))]
    crate stages: EnumMap<ShaderStage, Option<Arc<ShaderSpec>>>,
    crate cull_mode: vk::CullModeFlags,
    crate wireframe: bool,
    crate depth_test: bool,
    crate depth_write: bool,
    crate depth_cmp_op: vk::CompareOp,
    crate depth_bias: bool,
    // We have no use case yet for distinct color blending states.
    crate blend_state: AttachmentBlendDesc,
    #[derivative(Hash(hash_with = "byte_eq"))]
    #[derivative(Hash(hash_with = "byte_hash"))]
    crate blend_consts: [f32; 4],
}
impl Eq for GraphicsPipelineDesc {}

fn color_write_mask_all() -> vk::ColorComponentFlags {
    vk::ColorComponentFlags::R_BIT
        | vk::ColorComponentFlags::G_BIT
        | vk::ColorComponentFlags::B_BIT
        | vk::ColorComponentFlags::A_BIT
}

#[derive(Clone, Copy, Debug, Derivative, Eq, Hash, PartialEq)]
#[derivative(Default)]
crate struct AttachmentBlendDesc {
    crate blend_enable: bool,
    crate src_color_blend_factor: vk::BlendFactor,
    crate dst_color_blend_factor: vk::BlendFactor,
    crate color_blend_op: vk::BlendOp,
    crate src_alpha_blend_factor: vk::BlendFactor,
    crate dst_alpha_blend_factor: vk::BlendFactor,
    crate alpha_blend_op: vk::BlendOp,
    // I've been left scratching my head a few times after forgetting to
    // fill this field out.
    #[derivative(Default(value = "color_write_mask_all()"))]
    crate color_write_mask: vk::ColorComponentFlags,
}

impl From<AttachmentBlendDesc> for vk::PipelineColorBlendAttachmentState {
    fn from(desc: AttachmentBlendDesc) -> Self {
        Self {
            blend_enable: bool32(desc.blend_enable),
            src_color_blend_factor: desc.src_color_blend_factor,
            dst_color_blend_factor: desc.dst_color_blend_factor,
            color_blend_op: desc.color_blend_op,
            src_alpha_blend_factor: desc.src_alpha_blend_factor,
            dst_alpha_blend_factor: desc.dst_alpha_blend_factor,
            alpha_blend_op: desc.alpha_blend_op,
            color_write_mask: desc.color_write_mask,
        }
    }
}

impl Drop for PipelineLayout {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe { dt.destroy_pipeline_layout(self.inner, ptr::null()); }
    }
}

impl PipelineLayout {
    crate unsafe fn new(
        device: Arc<Device>,
        set_layouts: Vec<Arc<DescriptorSetLayout>>,
    ) -> Self {
        let dt = &*device.table;
        let cap = device.limits().max_bound_descriptor_sets as usize;
        assert!(set_layouts.len() < cap);

        let vk_set_layouts: Vec<_> = set_layouts.iter()
            .map(|layout| layout.inner())
            .collect();
        let create_info = vk::PipelineLayoutCreateInfo {
            set_layout_count: vk_set_layouts.len() as _,
            p_set_layouts: vk_set_layouts.as_ptr(),
            ..Default::default()
        };
        let mut inner = vk::null();
        dt.create_pipeline_layout(&create_info, ptr::null(), &mut inner)
            .check().unwrap();

        PipelineLayout {
            device,
            inner,
            set_layouts,
        }
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    crate fn inner(&self) -> vk::PipelineLayout {
        self.inner
    }

    crate fn set_layouts(&self) -> &[Arc<DescriptorSetLayout>] {
        &self.set_layouts
    }
}

impl Drop for GraphicsPipeline {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe { dt.destroy_pipeline(self.inner, ptr::null()); }
    }
}

impl GraphicsPipeline {
    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    crate fn inner(&self) -> vk::Pipeline {
        self.inner
    }

    crate fn layout(&self) -> &Arc<PipelineLayout> {
        &self.layout
    }

    crate fn pass(&self) -> &Arc<RenderPass> {
        &self.pass
    }

    crate fn subpass(&self) -> u32 {
        self.subpass
    }
}

impl GraphicsPipelineDesc {
    crate fn new(
        subpass: Subpass,
        layout: Arc<PipelineLayout>,
        vertex_layout: Arc<VertexLayout>,
    ) -> Self {
        Self {
            subpass,
            layout,
            vertex_layout,
            stages: Default::default(),
            cull_mode: Default::default(),
            wireframe: Default::default(),
            depth_test: Default::default(),
            depth_write: Default::default(),
            depth_cmp_op: Default::default(),
            depth_bias: Default::default(),
            blend_state: Default::default(),
            blend_consts: Default::default(),
        }
    }
}

type StageMap = EnumMap<ShaderStage, Option<Arc<ShaderSpec>>>;

// TODO: Could generalize this with a better `AsPtr` trait.
#[inline(always)]
fn to_ptr<T, P: std::ops::Deref<Target = T>>(x: &Option<P>) -> *const T {
    x.as_ref().map(|p| &**p as *const T).unwrap_or(ptr::null())
}

unsafe fn create_graphics_pipeline(
    device: Arc<Device>,
    desc: GraphicsPipelineDesc,
) -> Result<Arc<GraphicsPipeline>, ()> {
    let layout = Arc::clone(&desc.layout);

    let have = |stage| desc.stages[stage].is_some();
    assert!(have(ShaderStage::Vertex));
    assert!(have(ShaderStage::TessControl) == have(ShaderStage::TessEval));
    // TODO: Tessellation
    assert!(!have(ShaderStage::TessControl));
    // TODO: Use reflection to validate shader bindings against layout
    let stages: Vec<_> = desc.stages.iter().filter_map(|(stage, spec)| {
        let spec = spec.as_ref()?;
        let shader = spec.shader();
        assert_eq!(stage, shader.stage());
        Some(vk::PipelineShaderStageCreateInfo {
            module: shader.module(),
            stage: stage.into(),
            p_name: shader.entry_cstr().as_ptr(),
            p_specialization_info: spec.spec_info(),
            ..Default::default()
        })
    }).collect();

    let vertex_shader = desc.stages[ShaderStage::Vertex].as_ref().unwrap();
    let bindings = desc.vertex_layout.bindings();
    let attrs = desc.vertex_layout.input_attrs(vertex_shader.shader())?;
    let vertex_input = vk::PipelineVertexInputStateCreateInfo {
        vertex_binding_description_count: bindings.len() as _,
        p_vertex_binding_descriptions: bindings.as_ptr(),
        vertex_attribute_description_count: attrs.len() as _,
        p_vertex_attribute_descriptions: attrs.as_ptr(),
        ..Default::default()
    };

    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo {
        topology: desc.vertex_layout.topology,
        ..Default::default()
    };

    assert!(desc.stages[ShaderStage::TessControl].is_none());
    assert!(desc.stages[ShaderStage::TessEval].is_none());

    let viewport = vk::PipelineViewportStateCreateInfo {
        viewport_count: 1,
        scissor_count: 1,
        // Scissors set dynamically
        ..Default::default()
    };

    let rasterization = vk::PipelineRasterizationStateCreateInfo {
        polygon_mode:
            if desc.wireframe { vk::PolygonMode::LINE }
            else { vk::PolygonMode::FILL },
        cull_mode: desc.cull_mode,
        front_face: vk::FrontFace::COUNTER_CLOCKWISE,
        depth_bias_enable: bool32(desc.depth_bias),
        // Depth bias parameters set dynamically
        line_width: 1.0,
        ..Default::default()
    };

    let multisample = vk::PipelineMultisampleStateCreateInfo {
        rasterization_samples: desc.subpass.samples(),
        ..Default::default()
    };

    let depth = vk::PipelineDepthStencilStateCreateInfo {
        depth_test_enable: bool32(desc.depth_test),
        depth_write_enable: bool32(desc.depth_write),
        depth_compare_op: desc.depth_cmp_op,
        min_depth_bounds: 1.0,
        max_depth_bounds: 0.0,
        ..Default::default()
    };

    let attachment: vk::PipelineColorBlendAttachmentState =
        desc.blend_state.into();
    let attachments = vec![attachment; desc.subpass.color_attchs().len()];
    let color_blend = vk::PipelineColorBlendStateCreateInfo {
        attachment_count: attachments.len() as _,
        p_attachments: attachments.as_ptr(),
        blend_constants: desc.blend_consts,
        ..Default::default()
    };

    let dynamic_states = [
        vk::DynamicState::VIEWPORT,
        vk::DynamicState::SCISSOR,
        vk::DynamicState::DEPTH_BIAS,
    ];
    let dynamic = vk::PipelineDynamicStateCreateInfo {
        dynamic_state_count: dynamic_states.len() as _,
        p_dynamic_states: dynamic_states.as_ptr(),
        ..Default::default()
    };

    let create_info = vk::GraphicsPipelineCreateInfo {
        stage_count: stages.len() as _,
        p_stages: stages.as_ptr(),
        p_vertex_input_state: &vertex_input,
        p_input_assembly_state: &input_assembly,
        p_tessellation_state: ptr::null(),
        p_viewport_state: &viewport,
        p_rasterization_state: &rasterization,
        p_multisample_state: &multisample,
        p_depth_stencil_state: &depth,
        p_color_blend_state: &color_blend,
        p_dynamic_state: &dynamic,
        layout: layout.inner(),
        render_pass: desc.subpass.pass().inner(),
        subpass: desc.subpass.index(),
        ..Default::default()
    };
    let create_infos = std::slice::from_ref(&create_info);

    let dt = &*device.table;
    let mut pipeline = vk::null();
    dt.create_graphics_pipelines(
        vk::null(),
        create_infos.len() as _,
        create_infos.as_ptr(),
        ptr::null(),
        &mut pipeline,
    );

    Ok(Arc::new(GraphicsPipeline {
        device,
        inner: pipeline,
        layout,
        pass: Arc::clone(&desc.subpass.pass()),
        subpass: desc.subpass.index(),
    }))
}

#[cfg(test)]
mod tests {
    use enum_map::enum_map;
    use crate::*;
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        use VertexAttrName as Attr;

        let device = Arc::clone(&vars.swapchain.device);
        let set_layouts = BuiltinSetLayouts::new(&device);
        let shaders = BuiltinShaders::new(&device);

        let pass = ScreenPass::new(Arc::clone(&device));
        let subpass = pass.color.clone();

        let layout = Arc::new(PipelineLayout::new(
            Arc::clone(&device),
            vec![
                Arc::clone(&set_layouts.example_globals),
                Arc::clone(&set_layouts.example_instances),
            ],
        ));

        let attrs = &[
            (Attr::Position,    vk::Format::R32G32B32_SFLOAT,   12),
            (Attr::Normal,      vk::Format::R32G32B32_SFLOAT,   12),
        ];
        let vert_layout = Arc::new(VertexLayout::from_attrs_unpacked(attrs));

        let mut desc = GraphicsPipelineDesc::new(subpass, layout, vert_layout);
        desc.cull_mode = vk::CullModeFlags::BACK_BIT;

        desc.stages[ShaderStage::Vertex] =
            Some(Arc::new(ShaderSpec::new(Arc::clone(&shaders.example_vert))));
        desc.stages[ShaderStage::Fragment] =
            Some(Arc::new(ShaderSpec::new(Arc::clone(&shaders.example_frag))));

        let _pipeline = create_graphics_pipeline(Arc::clone(&device), desc);
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];