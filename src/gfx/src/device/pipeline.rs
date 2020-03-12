use std::borrow::Cow;
use std::convert::TryInto;
use std::fmt::Debug;
use std::ptr;
use std::sync::Arc;

use derivative::Derivative;
use enum_map::EnumMap;
use log::trace;

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
    // TODO: Arc<Desc> so hashmap and pipeline share the same object
    desc: GraphicsPipelineDesc,
}

#[derive(Clone, Debug, Derivative)]
#[derivative(Hash, PartialEq)]
crate struct GraphicsPipelineDesc {
    crate subpass: Subpass,
    // TODO: This should maybe just be Vec<Arc<SetLayout>>
    #[derivative(Hash(hash_with = "ptr_hash"))]
    #[derivative(PartialEq(compare_with = "ptr_eq"))]
    crate layout: Arc<PipelineLayout>,
    // TODO: This needs encapsulation so it can be auto-computed
    crate vertex_layout: VertexInputLayout,
    #[derivative(Hash(hash_with = "byte_hash"))]
    #[derivative(PartialEq(compare_with = "byte_eq"))]
    crate stages: EnumMap<ShaderStage, Option<Arc<ShaderSpec>>>,
    crate cull_mode: vk::CullModeFlags,
    crate wireframe: bool,
    crate depth_test: bool,
    crate depth_write: bool,
    crate depth_cmp_op: vk::CompareOp,
    crate depth_bias: bool,
    // We have no use case yet for multiple color blending states.
    crate blend_state: vk::PipelineColorBlendAttachmentState,
    #[derivative(Hash(hash_with = "byte_eq"))]
    #[derivative(Hash(hash_with = "byte_hash"))]
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
    crate fn new(
        device: Arc<Device>,
        set_layouts: Vec<Arc<DescriptorSetLayout>>,
    ) -> Self {
        unsafe { Self::unsafe_new(device, set_layouts) }
    }

    unsafe fn unsafe_new(
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

    crate fn desc(&self) -> &GraphicsPipelineDesc {
        &self.desc
    }

    crate fn layout(&self) -> &Arc<PipelineLayout> {
        &self.desc.layout
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
        self.desc.vertex_stage().unwrap()
    }
}

fn color_write_mask_all() -> vk::ColorComponentFlags {
    vk::ColorComponentFlags::R_BIT
        | vk::ColorComponentFlags::G_BIT
        | vk::ColorComponentFlags::B_BIT
        | vk::ColorComponentFlags::A_BIT
}

impl GraphicsPipelineDesc {
    crate fn new(subpass: Subpass, layout: Arc<PipelineLayout>) -> Self {
        Self {
            subpass,
            layout,
            vertex_layout: Default::default(),
            stages: Default::default(),
            cull_mode: vk::CullModeFlags::BACK_BIT,
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

    crate fn vertex_stage(&self) -> Option<&Arc<ShaderSpec>> {
        self.stages[ShaderStage::Vertex].as_ref()
    }
}

type StageMap = EnumMap<ShaderStage, Option<Arc<ShaderSpec>>>;

unsafe fn create_graphics_pipeline(
    device: Arc<Device>,
    desc: GraphicsPipelineDesc,
) -> Result<Arc<GraphicsPipeline>, ()> {
    trace!("creating graphics pipeline: {:?}", desc);

    let layout = Arc::clone(&desc.layout);

    let have = |stage| desc.stages[stage].is_some();
    assert!(have(ShaderStage::Vertex));
    assert_eq!(have(ShaderStage::TessControl), have(ShaderStage::TessEval));
    // TODO: Tessellation
    assert!(!have(ShaderStage::TessControl));
    let mut stages = desc.stages.values().filter_map(|stage| stage.as_ref());
    let mut stage0 = stages.next().unwrap();
    for stage1 in stages {
        assert_eq!(
            stage0.shader().outputs(),
            stage1.shader().inputs(),
            "{:?}, {:?}", stage0, stage1,
        );
        stage0 = stage1;
    }
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

    let vertex_shader = desc.vertex_stage().unwrap().shader();
    let vertex_layout = desc.vertex_layout;

    for &input in vertex_shader.inputs().iter() {
        // TODO: Check that format is compatible with input.ty
        let _attr = vertex_layout.attrs[input.try_into().unwrap()].unwrap();
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
        rasterization_samples: desc.subpass.samples().into(),
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

    Ok(Arc::new(GraphicsPipeline {
        device,
        inner: pipeline,
        desc,
    }))
}

macro_rules! pipeline_cache {
    (
        name: $name:ident,
        pipeline: $pipeline:ident,
        desc: $desc:ident,
        factory: $factory:ident$(,)?
    ) => {
        #[derive(Debug)]
        crate struct $name {
            device: Arc<Device>,
            inner: StagedCache<$desc, Arc<$pipeline>>,
        }

        impl $name {
            crate fn new(device: Arc<Device>) -> Self {
                $name {
                    device,
                    inner: Default::default(),
                }
            }

            crate fn commit(&mut self) {
                self.inner.commit();
            }

            crate fn get_committed(&self, desc: &$desc) ->
                Option<&Arc<$pipeline>>
            {
                self.inner.get_committed(desc)
            }

            crate unsafe fn get_or_create(&self, desc: &$desc) ->
                Cow<Arc<$pipeline>>
            {
                self.inner.get_or_insert_with(desc, || {
                    $factory(Arc::clone(&self.device), desc.clone()).unwrap()
                })
            }
        }
    }
}

pipeline_cache! {
    name: GraphicsPipelineCache,
    pipeline: GraphicsPipeline,
    desc: GraphicsPipelineDesc,
    factory: create_graphics_pipeline,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use crate::*;
    use super::*;

    fn trivial_pipe_desc(
        globals: &Globals,
        pass: &TrivialPass,
        trivial: &TrivialRenderer,
    ) -> GraphicsPipelineDesc {
        let mut desc = GraphicsPipelineDesc::new(
            pass.subpass.clone(),
            Arc::clone(&trivial.pipeline_layout()),
        );

        let shaders = &globals.shaders;
        desc.stages[ShaderStage::Vertex] =
            Some(Arc::new(Arc::clone(&shaders.trivial_vert).into()));
        desc.stages[ShaderStage::Fragment] =
            Some(Arc::new(Arc::clone(&shaders.trivial_frag).into()));

        desc.depth_test = false;

        desc
    }

    unsafe fn create_test(vars: testing::TestVars) {
        let device = Arc::clone(vars.device());
        let state = SystemState::new(Arc::clone(&device));
        let globals = Arc::new(Globals::new(&state));
        let pass = TrivialPass::new(Arc::clone(&device));
        let trivial = TrivialRenderer::new(&state, Arc::clone(&globals));

        let desc = trivial_pipe_desc(&globals, &pass, &trivial);
        let _pipeline = create_graphics_pipeline(Arc::clone(&device), desc);
    }

    unsafe fn cache_test(vars: crate::testing::TestVars) {
        let device = Arc::clone(vars.device());
        let mut state = SystemState::new(Arc::clone(&device));
        let globals = Arc::new(Globals::new(&state));
        let pass = TrivialPass::new(Arc::clone(&device));
        let trivial = TrivialRenderer::new(&state, Arc::clone(&globals));

        let cache = &mut state.gfx_pipes;

        let mut desc = trivial_pipe_desc(&globals, &pass, &trivial);
        let _pipe0 = Arc::clone(&cache.get_or_create(&desc));

        desc.depth_test = true;
        let pipe1 = Arc::clone(&cache.get_or_create(&desc));

        cache.commit();

        assert!(Arc::ptr_eq(cache.get_committed(&desc).unwrap(), &pipe1));
    }

    unit::declare_tests![
        create_test,
        cache_test,
    ];
}

unit::collect_tests![tests];
