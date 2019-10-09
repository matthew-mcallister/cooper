use std::ptr;
use std::sync::Arc;

use fnv::FnvHashMap;
use prelude::*;

use crate::*;

// TODO: There's probably some way to automate using these.
#[derive(Debug)]
pub struct PipelineLayout {
    pub inner: vk::PipelineLayout,
    pub set_layouts: Vec<String>,
}

#[derive(Debug)]
pub struct PipelineLayoutManager {
    crate device: Arc<Device>,
    set_layouts: Arc<DescriptorSetLayoutManager>,
    layouts: FnvHashMap<String, PipelineLayout>,
}

impl Drop for PipelineLayoutManager {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            for layout in self.layouts.values() {
                dt.destroy_pipeline_layout(layout.inner, ptr::null());
            }
        }
    }
}

impl PipelineLayoutManager {
    pub fn new(set_layouts: Arc<DescriptorSetLayoutManager>) ->
        PipelineLayoutManager
    {
        PipelineLayoutManager {
            device: Arc::clone(&set_layouts.device),
            set_layouts,
            layouts: Default::default(),
        }
    }

    pub unsafe fn create_layout(
        &mut self,
        name: String,
        set_layouts: Vec<String>,
    ) {
        let dt = &*self.device.table;

        let vk_set_layouts: Vec<_> = set_layouts.iter()
            .map(|id| self.set_layouts.get(&id).inner)
            .collect();
        let create_info = vk::PipelineLayoutCreateInfo {
            set_layout_count: vk_set_layouts.len() as _,
            p_set_layouts: vk_set_layouts.as_ptr(),
            ..Default::default()
        };
        let mut inner = vk::null();
        dt.create_pipeline_layout(&create_info, ptr::null(), &mut inner)
            .check().unwrap();

        let layout = PipelineLayout {
            inner,
            set_layouts,
        };
        insert_unique!(self.layouts, name, layout);
    }

    pub unsafe fn get(&self, key: impl AsRef<str>) -> &PipelineLayout {
        &self.layouts[key.as_ref()]
    }
}

/// Does the heavy lifting in creating graphics pipelines.
pub trait GraphicsPipelineFactory {
    /// Input to the pipeline creation routine and key to the created
    /// pipeline cache. Parameterizes pipelines in a functional,
    /// application-specific manner.
    type Desc: Clone + std::hash::Hash + Eq;

    unsafe fn create_pipeline(&mut self, desc: &Self::Desc) ->
        GraphicsPipeline;
}

#[derive(Debug)]
pub struct GraphicsPipeline {
    pub inner: vk::Pipeline,
    pub layout: String,
    pub render_pass: String,
    pub subpass: String,
}

/// Basically a cache of pipelines.
#[derive(Debug)]
pub struct GraphicsPipelineManager<F: GraphicsPipelineFactory> {
    crate device: Arc<Device>,
    factory: F,
    pipelines: FnvHashMap<F::Desc, Arc<GraphicsPipeline>>,
}

impl<F: GraphicsPipelineFactory> Drop for GraphicsPipelineManager<F> {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            for pipeline in self.pipelines.values() {
                dt.destroy_pipeline(pipeline.inner, ptr::null());
            }
        }
    }
}

impl<F: GraphicsPipelineFactory> GraphicsPipelineManager<F> {
    pub unsafe fn new(device: Arc<Device>, factory: F) -> Self {
        GraphicsPipelineManager {
            device,
            factory,
            pipelines: Default::default(),
        }
    }

    pub unsafe fn get(&mut self, desc: &F::Desc) -> &Arc<GraphicsPipeline> {
        // Work around for borrow check limitation
        // Related to: https://github.com/rust-lang/polonius
        let pipelines: &FnvHashMap<F::Desc, Arc<GraphicsPipeline>> =
            &*(&self.pipelines as *const _);

        if let Some(pl) = pipelines.get(desc) {
            // Pipeline exists; don't create it.
            return pl;
        }

        let pipeline = self.factory.create_pipeline(desc);

        insert_unique!(self.pipelines, desc.clone(), Arc::new(pipeline));
        &self.pipelines[desc]
    }
}

#[cfg(test)]
crate unsafe fn create_test_pipe_layouts(vars: &testing::TestVars) ->
    (Arc<DescriptorSetLayoutManager>, Arc<PipelineLayoutManager>)
{
    let set_layouts = create_test_set_layouts(vars);
    let mut pipe_layouts =
        PipelineLayoutManager::new(Arc::clone(&set_layouts));

    pipe_layouts.create_layout(
        "std_mesh".to_owned(),
        vec!["scene_globals".to_owned(), "material".to_owned()],
    );

    (set_layouts, Arc::new(pipe_layouts))
}

#[cfg(test)]
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
crate enum TestPipelineDesc {
    Cube,
}

#[cfg(test)]
#[derive(Debug)]
crate struct TestPipelineFactory {
    swapchain: Arc<Swapchain>,
    render_passes: Arc<RenderPassManager>,
    shaders: Arc<ShaderManager>,
    set_layouts: Arc<DescriptorSetLayoutManager>,
    pipe_layouts: Arc<PipelineLayoutManager>,
}

#[cfg(test)]
impl GraphicsPipelineFactory for TestPipelineFactory {
    type Desc = TestPipelineDesc;

    // Simple, copypasta pipeline creation
    unsafe fn create_pipeline(&mut self, desc: &Self::Desc) -> GraphicsPipeline
    {
        let dt = &*self.swapchain.device.table;

        let (stages, layout_id) = match *desc {
            TestPipelineDesc::Cube => {
                let vert = self.shaders.get("example_vert");
                let frag = self.shaders.get("example_frag");

                let vert_stage = vk::PipelineShaderStageCreateInfo {
                    stage: vk::ShaderStageFlags::VERTEX_BIT,
                    module: vert.inner,
                    p_name: vert.entry().as_ptr(),
                    ..Default::default()
                };
                let frag_stage = vk::PipelineShaderStageCreateInfo {
                    stage: vk::ShaderStageFlags::FRAGMENT_BIT,
                    module: frag.inner,
                    p_name: frag.entry().as_ptr(),
                    ..Default::default()
                };
                let stages = vec![vert_stage, frag_stage];

                (stages, "std_mesh")
            },
        };
        let layout = self.pipe_layouts.get(layout_id);

        let (render_pass_id, subpass_id) = match *desc {
            TestPipelineDesc::Cube => ("forward", "lighting"),
        };
        let render_pass = self.render_passes.get(render_pass_id);
        let subpass = render_pass.subpasses[subpass_id];

        let (vertex_input_state, input_assembly_state) = match *desc {
            TestPipelineDesc::Cube => {
                let bindings = [
                    // Position
                    vk::VertexInputBindingDescription {
                        binding: 0,
                        stride: std::mem::size_of::<[f32; 3]>() as _,
                        input_rate: vk::VertexInputRate::VERTEX,
                    },
                    // Normal
                    vk::VertexInputBindingDescription {
                        binding: 1,
                        stride: std::mem::size_of::<[f32; 3]>() as _,
                        input_rate: vk::VertexInputRate::VERTEX,
                    },
                ];
                let attrs = [
                    // Position
                    vk::VertexInputAttributeDescription {
                        location: 0,
                        binding: 0,
                        format: vk::Format::R32G32B32_SFLOAT,
                        offset: 0,
                    },
                    // Normal
                    vk::VertexInputAttributeDescription {
                        location: 1,
                        binding: 1,
                        format: vk::Format::R32G32B32_SFLOAT,
                        offset: 0,
                    },
                ];
				let vertex_input = vk::PipelineVertexInputStateCreateInfo {
                    vertex_binding_description_count: bindings.len() as _,
                    p_vertex_binding_descriptions: bindings.as_ptr(),
                    vertex_attribute_description_count: attrs.len() as _,
                    p_vertex_attribute_descriptions: attrs.as_ptr(),
                    ..Default::default()
                };
                let input_assembly = vk::PipelineInputAssemblyStateCreateInfo {
                    topology: vk::PrimitiveTopology::TRIANGLE_STRIP,
                    ..Default::default()
                };
                (vertex_input, input_assembly)
            },
        };

        let viewports = [self.swapchain.viewport()];
        let scissors = [self.swapchain.rect()];
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
            layout: layout.inner,
            render_pass: render_pass.inner,
            subpass,
            ..Default::default()
        };
        let create_infos = std::slice::from_ref(&create_info);

        let mut inner = vk::null();
        let pipelines = std::slice::from_mut(&mut inner);
        dt.create_graphics_pipelines(
            vk::null(),                 // pipelineCache
            create_infos.len() as _,    // createInfoCount
            create_infos.as_ptr(),      // pCreateInfos
            ptr::null(),                // pAllocator
            pipelines.as_mut_ptr(),     // pPipelines
        ).check().unwrap();

        GraphicsPipeline {
            inner,
            layout: layout_id.to_owned(),
            render_pass: render_pass_id.to_owned(),
            subpass: subpass_id.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use vk::traits::*;
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let swapchain = Arc::clone(&vars.swapchain);
        let device = Arc::clone(&swapchain.device);

        let (render_passes, _, _) = create_test_render_passes(&vars);
        let shaders = create_test_shaders(&vars);
        let (set_layouts, pipe_layouts) = create_test_pipe_layouts(&vars);

        let factory = TestPipelineFactory {
            swapchain: Arc::clone(&swapchain),
            render_passes: Arc::new(render_passes),
            shaders,
            set_layouts,
            pipe_layouts,
        };

        let mut pipe_man = GraphicsPipelineManager::new(device, factory);

        let pipe1 = Arc::clone(pipe_man.get(&TestPipelineDesc::Cube));
        let pipe2 = Arc::clone(pipe_man.get(&TestPipelineDesc::Cube));
        assert_eq!(&*pipe1 as *const _, &*pipe2 as *const _);
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
