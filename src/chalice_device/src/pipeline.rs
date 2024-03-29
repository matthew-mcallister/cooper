use std::borrow::Cow;
use std::fmt::Debug;
use std::ptr;
use std::sync::Arc;

use base::PartialEnumMap;
use derivative::Derivative;
use enum_map::Enum;
use log::trace;
use more_asserts::assert_lt;

use crate::*;

/// Returns a vk::PipelineColorBlendAttachmentState with a default
/// color write mask set.
pub fn default_color_blend_state() -> vk::PipelineColorBlendAttachmentState {
    vk::PipelineColorBlendAttachmentState {
        color_write_mask: vk::ColorComponentFlags::R_BIT
            | vk::ColorComponentFlags::G_BIT
            | vk::ColorComponentFlags::B_BIT
            | vk::ColorComponentFlags::A_BIT,
        ..Default::default()
    }
}

#[derive(Debug)]
pub struct PipelineLayout {
    device: Arc<Device>,
    inner: vk::PipelineLayout,
    set_layouts: SmallVec<Arc<DescriptorSetLayout>, 4>,
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct PipelineLayoutDesc {
    pub set_layouts: SmallVec<Arc<DescriptorSetLayout>, 4>,
}

pub type ShaderStageMap = PartialEnumMap<ShaderStage, Arc<ShaderSpec>>;

#[derive(Debug)]
pub struct GraphicsPipeline {
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
pub struct GraphicsPipelineDesc {
    pub subpass: Subpass,
    pub layout: PipelineLayoutDesc,
    pub vertex_layout: VertexInputLayout,
    pub stages: ShaderStageMap,
    pub cull_mode: CullMode,
    pub wireframe: bool,
    pub depth_test: bool,
    pub depth_write: bool,
    pub depth_cmp_op: vk::CompareOp,
    pub depth_bias: bool,
    // We have no use case yet for multiple color blending states.
    pub blend_state: vk::PipelineColorBlendAttachmentState,
    #[derivative(Hash(hash_with = "byte_hash"))]
    #[derivative(PartialEq(compare_with = "byte_eq"))]
    pub blend_consts: [f32; 4],
}
impl Eq for GraphicsPipelineDesc {}

impl Drop for PipelineLayout {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            dt.destroy_pipeline_layout(self.inner, ptr::null());
        }
    }
}

impl PipelineLayout {
    pub fn new(device: Arc<Device>, desc: PipelineLayoutDesc) -> Self {
        let dt = &*device.table;
        let set_layouts = desc.set_layouts;
        let cap = device.limits().max_bound_descriptor_sets as usize;
        assert!(set_layouts.len() < cap);

        let vk_set_layouts: SmallVec<_, 4> =
            set_layouts.iter().map(|layout| layout.inner()).collect();
        let create_info = vk::PipelineLayoutCreateInfo {
            set_layout_count: vk_set_layouts.len() as _,
            p_set_layouts: vk_set_layouts.as_ptr(),
            ..Default::default()
        };
        let mut inner = vk::null();
        unsafe {
            dt.create_pipeline_layout(&create_info, ptr::null(), &mut inner)
                .check()
                .unwrap();
        }

        PipelineLayout {
            device,
            inner,
            set_layouts,
        }
    }

    #[inline]
    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    #[inline]
    pub fn inner(&self) -> vk::PipelineLayout {
        self.inner
    }

    #[inline]
    pub fn set_layouts(&self) -> &[Arc<DescriptorSetLayout>] {
        &self.set_layouts
    }
}

impl Drop for GraphicsPipeline {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            dt.destroy_pipeline(self.inner, ptr::null());
        }
    }
}

impl GraphicsPipeline {
    unsafe fn new(layout: Arc<PipelineLayout>, desc: GraphicsPipelineDesc) -> Self {
        create_graphics_pipeline(layout, desc)
    }

    #[inline]
    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    #[inline]
    pub fn inner(&self) -> vk::Pipeline {
        self.inner
    }

    #[inline]
    pub fn desc(&self) -> &GraphicsPipelineDesc {
        &self.desc
    }

    #[inline]
    pub fn layout(&self) -> &Arc<PipelineLayout> {
        &self.layout
    }

    #[inline]
    pub fn vertex_layout(&self) -> &VertexInputLayout {
        &self.desc.vertex_layout
    }

    #[inline]
    pub fn pass(&self) -> &Arc<RenderPass> {
        &self.desc.subpass.pass()
    }

    #[inline]
    pub fn subpass(&self) -> &Subpass {
        &self.desc.subpass
    }

    #[inline]
    pub fn vertex_stage(&self) -> &Arc<ShaderSpec> {
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
    #[inline]
    pub fn new(subpass: Subpass) -> Self {
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

    #[inline]
    pub fn vertex_stage(&self) -> &Arc<ShaderSpec> {
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
        layout
            .set_layouts
            .iter()
            .zip(desc.layout.set_layouts.iter())
            .all(|(layout, desc)| Arc::ptr_eq(layout, desc)),
        "layout: {:?}, desc: {:?}",
        layout.set_layouts,
        desc.layout.set_layouts,
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
            "{:?}, {:?}",
            stage0,
            stage1,
        );
        stage0 = stage1;
    }
    let stages: Vec<_> = desc
        .stages
        .iter()
        .map(|(stage, spec)| {
            let shader = spec.shader();
            assert_eq!(stage, shader.stage());
            vk::PipelineShaderStageCreateInfo {
                module: shader.module(),
                stage: stage.into(),
                p_name: shader.entry_cstr().as_ptr(),
                p_specialization_info: spec.spec_info(),
                ..Default::default()
            }
        })
        .collect();

    let vertex_shader = desc.vertex_stage().shader();
    let vertex_layout = &desc.vertex_layout;

    let attrs = &vertex_layout.attributes;
    for attr in attrs.windows(2) {
        assert_lt!(attr[0].location, attr[1].location);
    }

    for &location in vertex_shader.inputs().iter() {
        // TODO: Check that format is compatible with input.ty
        assert!(vertex_layout
            .attributes
            .iter()
            .any(|attr| attr.location == location));
    }

    let vertex_input = vk::PipelineVertexInputStateCreateInfo {
        vertex_binding_description_count: vertex_layout.bindings.len() as _,
        p_vertex_binding_descriptions: vertex_layout.bindings.as_ptr(),
        vertex_attribute_description_count: vertex_layout.attributes.len() as _,
        p_vertex_attribute_descriptions: vertex_layout.attributes.as_ptr(),
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
        polygon_mode: if desc.wireframe {
            vk::PolygonMode::LINE
        } else {
            vk::PolygonMode::FILL
        },
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
pub struct PipelineLayoutCache {
    device: Arc<Device>,
    inner: StagedCache<PipelineLayoutDesc, Arc<PipelineLayout>>,
}

/// Manages the creation, destruction, and lifetime of pipelines.
#[derive(Debug)]
pub struct PipelineCache {
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
        pub struct $name {
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

            unsafe fn get_or_create_committed(
                &mut self,
                layout: &Arc<PipelineLayout>,
                desc: &$desc,
            ) -> &Arc<$pipeline> {
                self.inner.get_or_insert_committed_with(desc, || {
                    Arc::new($pipeline::new(Arc::clone(layout), desc.clone()))
                })
            }

            fn get_committed(&self, desc: &$desc) -> Option<&Arc<$pipeline>> {
                self.inner.get_committed(desc)
            }

            unsafe fn get_or_create(
                &self,
                layout: &Arc<PipelineLayout>,
                desc: &$desc,
            ) -> Cow<Arc<$pipeline>> {
                self.inner.get_or_insert_with(desc, || {
                    Arc::new($pipeline::new(Arc::clone(layout), desc.clone()))
                })
            }
        }
    };
}

pipeline_cache! {
    name: GraphicsPipelineCache,
    pipeline: GraphicsPipeline,
    desc: GraphicsPipelineDesc,
}

impl PipelineLayoutCache {
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            device,
            inner: Default::default(),
        }
    }

    pub fn commit(&mut self) {
        self.inner.commit();
    }

    pub fn get_committed(&self, desc: &PipelineLayoutDesc) -> Option<&Arc<PipelineLayout>> {
        self.inner.get_committed(desc)
    }

    pub fn get_or_create_committed(&mut self, desc: &PipelineLayoutDesc) -> &Arc<PipelineLayout> {
        let device = &self.device;
        self.inner.get_or_insert_committed_with(desc, || {
            Arc::new(PipelineLayout::new(Arc::clone(&device), desc.clone()))
        })
    }

    pub fn get_or_create(&self, desc: &PipelineLayoutDesc) -> Cow<Arc<PipelineLayout>> {
        let device = &self.device;
        self.inner.get_or_insert_with(desc, || {
            Arc::new(PipelineLayout::new(Arc::clone(&device), desc.clone()))
        })
    }
}

impl PipelineCache {
    pub fn new(device: &Arc<Device>) -> Self {
        Self {
            layouts: PipelineLayoutCache::new(Arc::clone(device)),
            gfx: GraphicsPipelineCache::new(),
        }
    }

    pub fn commit(&mut self) {
        self.layouts.commit();
        self.gfx.commit();
    }

    pub fn get_committed_layout(&self, desc: &PipelineLayoutDesc) -> Option<&Arc<PipelineLayout>> {
        self.layouts.get_committed(desc)
    }

    pub fn get_or_create_committed_layout(
        &mut self,
        desc: &PipelineLayoutDesc,
    ) -> &Arc<PipelineLayout> {
        self.layouts.get_or_create_committed(desc)
    }

    pub fn get_committed_gfx(&self, desc: &GraphicsPipelineDesc) -> Option<&Arc<GraphicsPipeline>> {
        self.gfx.get_committed(desc)
    }

    pub unsafe fn get_or_create_committed_gfx(
        &mut self,
        desc: &GraphicsPipelineDesc,
    ) -> &Arc<GraphicsPipeline> {
        let layout = self.layouts.get_or_create_committed(&desc.layout);
        self.gfx.get_or_create_committed(&layout, desc)
    }

    pub fn get_or_create_layout(&self, desc: &PipelineLayoutDesc) -> Cow<Arc<PipelineLayout>> {
        self.layouts.get_or_create(desc)
    }

    pub unsafe fn get_or_create_gfx(
        &self,
        desc: &GraphicsPipelineDesc,
    ) -> Cow<Arc<GraphicsPipeline>> {
        let layout = self.layouts.get_or_create(&desc.layout);
        self.gfx.get_or_create(&layout, desc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;
    use std::sync::Arc;

    #[test]
    fn create() {
        let vars = TestVars::new();
        let device = vars.device();
        let resources = TestResources::new(device);
        let pass = TrivialPass::new(device);
        let trivial = TrivialRenderer::new(&resources);

        let mut desc = GraphicsPipelineDesc::new(pass.subpass.clone());
        trivial.init_pipe_desc(&mut desc);
        let layout = Arc::new(PipelineLayout::new(
            Arc::clone(&device),
            desc.layout.clone(),
        ));
        unsafe {
            let _pipeline = create_graphics_pipeline(layout, desc);
        }
    }

    #[test]
    fn cache() {
        let vars = TestVars::new();
        let device = vars.device();
        let resources = TestResources::new(device);
        let pass = TrivialPass::new(device);
        let trivial = TrivialRenderer::new(&resources);
        let mut cache = PipelineCache::new(device);

        unsafe {
            let mut desc = GraphicsPipelineDesc::new(pass.subpass.clone());
            trivial.init_pipe_desc(&mut desc);
            let _pipe0 = cache.get_or_create_gfx(&desc).into_owned();

            // FIXME: This causes a segfault WTF?
            desc.depth_test = true;
            let pipe1 = cache.get_or_create_gfx(&desc).into_owned();

            cache.commit();

            assert!(Arc::ptr_eq(
                cache.get_or_create_committed_gfx(&desc),
                &pipe1,
            ));
        }
    }
}
