use std::borrow::Cow;
use std::fmt::Debug;
use std::ptr;
use std::sync::Arc;

use base::PartialEnumMap;
use derivative::Derivative;
use enum_map::Enum;
use log::trace;

use crate::*;

#[derive(Debug)]
crate struct PipelineLayout {
    device: Arc<Device>,
    inner: vk::PipelineLayout,
    set_layouts: Vec<Arc<DescriptorSetLayout>>,
}

#[derive(Clone, Debug, Default, Derivative)]
#[derivative(Hash, PartialEq)]
crate struct PipelineLayoutDesc {
    #[derivative(Hash(hash_with = "slice_hash"))]
    #[derivative(PartialEq(compare_with = "slice_eq"))]
    crate set_layouts: Vec<Arc<DescriptorSetLayout>>,
    push_constants: Option<()>,
}
impl Eq for PipelineLayoutDesc {}

crate type ShaderStageMap = PartialEnumMap<ShaderStage, Arc<ShaderSpec>>;

#[derive(Debug)]
crate struct GraphicsPipeline {
    device: Arc<Device>,
    inner: vk::Pipeline,
    layout: Arc<PipelineLayout>,
    // TODO: Arc<Desc> so hashmap and pipeline share the same object
    desc: GraphicsPipelineDesc,
}

#[derive(Clone, Copy, Debug, Derivative, Enum, Eq, Hash, PartialEq)]
#[derivative(Default)]
pub enum CullMode {
    None = 0,
    Front = 1,
    #[derivative(Default)]
    Back = 2,
    FrontAndBack = 3,
}

#[derive(Clone, Debug, Derivative)]
#[derivative(Hash, PartialEq)]
crate struct GraphicsPipelineDesc {
    crate subpass: Subpass,
    crate layout: PipelineLayoutDesc,
    crate vertex_layout: VertexInputLayout,
    #[derivative(Hash(hash_with = "byte_hash"))]
    #[derivative(PartialEq(compare_with = "byte_eq"))]
    crate stages: ShaderStageMap,
    crate cull_mode: CullMode,
    crate wireframe: bool,
    crate depth_test: bool,
    crate depth_write: bool,
    crate depth_cmp_op: vk::CompareOp,
    crate depth_bias: bool,
    // We have no use case yet for multiple color blending states.
    crate blend_state: vk::PipelineColorBlendAttachmentState,
    #[derivative(Hash(hash_with = "byte_hash"))]
    #[derivative(PartialEq(compare_with = "byte_eq"))]
    crate blend_consts: [f32; 4],
}
impl Eq for GraphicsPipelineDesc {}

impl Drop for PipelineLayout {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe { dt.destroy_pipeline_layout(self.inner, ptr::null()); }
    }
}

impl PipelineLayout {
    crate fn new(device: Arc<Device>, desc: PipelineLayoutDesc) -> Self {
        unsafe { Self::unsafe_new(device, desc) }
    }

    unsafe fn unsafe_new(device: Arc<Device>, desc: PipelineLayoutDesc) ->
        Self
    {
        let dt = &*device.table;
        let set_layouts = desc.set_layouts;
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
    unsafe fn new(layout: Arc<PipelineLayout>, desc: GraphicsPipelineDesc) ->
        Self
    {
        create_graphics_pipeline(layout, desc)
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    crate fn inner(&self) -> vk::Pipeline {
        self.inner
    }

    crate fn desc(&self) -> &GraphicsPipelineDesc {
        &self.desc
    }

    crate fn layout(&self) -> &Arc<PipelineLayout> {
        &self.layout
    }

    crate fn vertex_layout(&self) -> &VertexInputLayout {
        &self.desc.vertex_layout
    }

    crate fn pass(&self) -> &Arc<RenderPass> {
        &self.desc.subpass.pass()
    }

    crate fn subpass(&self) -> &Subpass {
        &self.desc.subpass
    }

    crate fn vertex_stage(&self) -> &Arc<ShaderSpec> {
        self.desc.vertex_stage()
    }
}

fn color_write_mask_all() -> vk::ColorComponentFlags {
    vk::ColorComponentFlags::R_BIT
        | vk::ColorComponentFlags::G_BIT
        | vk::ColorComponentFlags::B_BIT
        | vk::ColorComponentFlags::A_BIT
}

impl GraphicsPipelineDesc {
    crate fn new(subpass: Subpass) -> Self {
        Self {
            subpass,
            layout: Default::default(),
            vertex_layout: Default::default(),
            stages: Default::default(),
            cull_mode: Default::default(),
            wireframe: Default::default(),
            depth_test: Default::default(),
            depth_write: Default::default(),
            depth_cmp_op: Default::default(),
            depth_bias: Default::default(),
            blend_state: vk::PipelineColorBlendAttachmentState {
                color_write_mask: color_write_mask_all(),
                ..Default::default()
            },
            blend_consts: Default::default(),
        }
    }

    crate fn vertex_stage(&self) -> &Arc<ShaderSpec> {
        &self.stages[ShaderStage::Vertex]
    }
}

unsafe fn create_graphics_pipeline(
    layout: Arc<PipelineLayout>,
    desc: GraphicsPipelineDesc,
) -> GraphicsPipeline {
    trace!("create_graphics_pipeline(desc: {:?})", desc);

    let device = Arc::clone(&layout.device);

    // TODO: This redundancy is unfortunate.
    assert!(
        layout.set_layouts.iter().zip(desc.layout.set_layouts.iter())
            .all(|(layout, desc)| Arc::ptr_eq(layout, desc)),
        "layout: {:?}, desc: {:?}",
        layout.set_layouts, desc.layout.set_layouts,
    );

    let have = |stage| desc.stages.contains_key(stage);
    assert!(have(ShaderStage::Vertex));
    assert_eq!(have(ShaderStage::TessControl), have(ShaderStage::TessEval));
    // TODO: Tessellation
    assert!(!have(ShaderStage::TessControl));
    let mut stages = desc.stages.values();
    let mut stage0 = stages.next().unwrap();
    for stage1 in stages {
        assert_eq!(
            stage0.shader().outputs(),
            stage1.shader().inputs(),
            "{:?}, {:?}", stage0, stage1,
        );
        stage0 = stage1;
    }
    let stages: Vec<_> = desc.stages.iter().map(|(stage, spec)| {
        let shader = spec.shader();
        assert_eq!(stage, shader.stage());
        vk::PipelineShaderStageCreateInfo {
            module: shader.module(),
            stage: stage.into(),
            p_name: shader.entry_cstr().as_ptr(),
            p_specialization_info: spec.spec_info(),
            ..Default::default()
        }
    }).collect();

    let vertex_shader = desc.vertex_stage().shader();
    let vertex_layout = &desc.vertex_layout;

    for &location in vertex_shader.inputs().iter() {
        // TODO: Check that format is compatible with input.ty
        assert!(vertex_layout.attributes.iter()
            .find(|attr| attr.location == location).is_some());
    }

    let bindings = vertex_layout.vk_bindings();
    let attrs = &vertex_layout.vk_attrs();

    let vertex_input = vk::PipelineVertexInputStateCreateInfo {
        vertex_binding_description_count: bindings.len() as _,
        p_vertex_binding_descriptions: bindings.as_ptr(),
        vertex_attribute_description_count: attrs.len() as _,
        p_vertex_attribute_descriptions: attrs.as_ptr(),
        ..Default::default()
    };

    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo {
        topology: vertex_layout.topology.into(),
        ..Default::default()
    };

    assert!(!desc.stages.contains_key(ShaderStage::TessControl));
    assert!(!desc.stages.contains_key(ShaderStage::TessEval));

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
        cull_mode: desc.cull_mode.into(),
        front_face: vk::FrontFace::COUNTER_CLOCKWISE,
        depth_bias_enable: bool32(desc.depth_bias),
        // Depth bias parameters set dynamically
        line_width: 1.0,
        ..Default::default()
    };

    let multisample = vk::PipelineMultisampleStateCreateInfo {
        rasterization_samples: desc.subpass.samples().into(),
        ..Default::default()
    };

    let depth = vk::PipelineDepthStencilStateCreateInfo {
        depth_test_enable: bool32(desc.depth_test),
        depth_write_enable: bool32(desc.depth_write),
        depth_compare_op: desc.depth_cmp_op,
        ..Default::default()
    };

    let attachment: vk::PipelineColorBlendAttachmentState = desc.blend_state;
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

    GraphicsPipeline {
        device,
        inner: pipeline,
        layout,
        desc,
    }
}

impl From<CullMode> for vk::CullModeFlags {
    fn from(mode: CullMode) -> Self {
        match mode {
            CullMode::None => Self::NONE,
            CullMode::Front => Self::FRONT_BIT,
            CullMode::Back => Self::BACK_BIT,
            CullMode::FrontAndBack => Self::FRONT_AND_BACK,
        }
    }
}

#[derive(Debug)]
crate struct PipelineLayoutCache {
    device: Arc<Device>,
    inner: StagedCache<PipelineLayoutDesc, Arc<PipelineLayout>>,
}

/// Manages the creation, destruction, and lifetime of pipelines.
#[derive(Debug)]
crate struct PipelineCache {
    layouts: PipelineLayoutCache,
    gfx: GraphicsPipelineCache,
}

macro_rules! pipeline_cache {
    (
        name: $name:ident,
        pipeline: $pipeline:ident,
        desc: $desc:ident,
    ) => {
        #[derive(Debug)]
        crate struct $name {
            inner: StagedCache<$desc, Arc<$pipeline>>,
        }

        impl $name {
            fn new() -> Self {
                Self {
                    inner: Default::default(),
                }
            }

            fn commit(&mut self) {
                self.inner.commit();
            }

            fn get_committed(&self, desc: &$desc) -> Option<&Arc<$pipeline>> {
                self.inner.get_committed(desc)
            }

            unsafe fn get_or_create(
                &self,
                layout: Arc<PipelineLayout>,
                desc: &$desc,
            ) -> Cow<Arc<$pipeline>> {
                self.inner.get_or_insert_with(desc, || {
                    Arc::new($pipeline::new(layout, desc.clone()))
                })
            }
        }
    }
}

pipeline_cache! {
    name: GraphicsPipelineCache,
    pipeline: GraphicsPipeline,
    desc: GraphicsPipelineDesc,
}

impl PipelineLayoutCache {
    crate fn new(device: Arc<Device>) -> Self {
        Self {
            device,
            inner: Default::default(),
        }
    }

    crate fn commit(&mut self) {
        self.inner.commit();
    }

    crate fn get_committed(&self, desc: &PipelineLayoutDesc) ->
        Option<&Arc<PipelineLayout>>
    {
        self.inner.get_committed(desc)
    }

    crate unsafe fn get_or_create(&self, desc: &PipelineLayoutDesc) ->
        Cow<Arc<PipelineLayout>>
    {
        self.inner.get_or_insert_with(desc, || Arc::new(PipelineLayout::new(
            Arc::clone(&self.device),
            desc.clone(),
        )))
    }
}

impl PipelineCache {
    crate fn new(device: &Arc<Device>) -> Self {
        Self {
            layouts: PipelineLayoutCache::new(Arc::clone(device)),
            gfx: GraphicsPipelineCache::new(),
        }
    }

    crate fn commit(&mut self) {
        self.layouts.commit();
        self.gfx.commit();
    }

    crate fn get_committed_layout(&self, desc: &PipelineLayoutDesc) ->
        Option<&Arc<PipelineLayout>>
    {
        self.layouts.get_committed(desc)
    }

    crate fn get_committed_gfx(&self, desc: &GraphicsPipelineDesc) ->
        Option<&Arc<GraphicsPipeline>>
    {
        self.gfx.get_committed(desc)
    }

    crate unsafe fn get_or_create_layout(&self, desc: &PipelineLayoutDesc) ->
        Cow<Arc<PipelineLayout>>
    {
        self.layouts.get_or_create(desc)
    }

    crate unsafe fn get_or_create_gfx(&self, desc: &GraphicsPipelineDesc) ->
        Cow<Arc<GraphicsPipeline>>
    {
        let layout = self.get_or_create_layout(&desc.layout);
        self.gfx.get_or_create(layout.into_owned(), desc)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use crate::*;
    use super::*;

    unsafe fn create_test(vars: testing::TestVars) {
        let device = vars.device();
        let state = SystemState::new(Arc::clone(&device));
        let heap = ImageHeap::new(Arc::clone(&device));
        let globals = Arc::new(Globals::new(&state, &heap));
        let pass = TrivialPass::new(Arc::clone(&device));
        let trivial = TrivialRenderer::new(&state, Arc::clone(&globals));

        let mut desc = GraphicsPipelineDesc::new(pass.subpass.clone());
        trivial.init_pipe_desc(&mut desc);
        let layout = Arc::new(PipelineLayout::new(
            Arc::clone(&device),
            desc.layout.clone(),
        ));
        let _pipeline = create_graphics_pipeline(layout, desc);
    }

    unsafe fn cache_test(vars: crate::testing::TestVars) {
        let device = Arc::clone(vars.device());
        let mut state = SystemState::new(Arc::clone(&device));
        let heap = ImageHeap::new(Arc::clone(&device));
        let globals = Arc::new(Globals::new(&state, &heap));
        let pass = TrivialPass::new(Arc::clone(&device));
        let trivial = TrivialRenderer::new(&state, Arc::clone(&globals));

        let cache = &mut state.pipelines;

        let mut desc = GraphicsPipelineDesc::new(pass.subpass.clone());
        trivial.init_pipe_desc(&mut desc);
        let _pipe0 = cache.get_or_create_gfx(&desc).into_owned();

        desc.depth_test = true;
        let pipe1 = cache.get_or_create_gfx(&desc).into_owned();

        cache.commit();

        assert!(Arc::ptr_eq(cache.get_committed_gfx(&desc).unwrap(), &pipe1));
    }

    unit::declare_tests![
        create_test,
        cache_test,
    ];
}

unit::collect_tests![tests];
