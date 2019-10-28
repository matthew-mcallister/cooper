use std::ptr;

use ccore::name::*;

use crate::*;

#[derive(Debug)]
crate struct PipelineLayout {
    inner: vk::PipelineLayout,
    set_layouts: Vec<Name>,
}

impl PipelineLayout {
    crate fn inner(&self) -> vk::PipelineLayout {
        self.inner
    }

    crate fn set_layouts(&self) -> &[Name] {
        &self.set_layouts
    }
}

crate unsafe fn create_pipeline_layout(
    core: &CoreData,
    set_layouts: Vec<Name>,
) -> PipelineLayout {
    let dt = &*core.device().table;

    let vk_set_layouts: Vec<_> = set_layouts.iter()
        .map(|&id| core.get_set_layout(id).inner())
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
        inner,
        set_layouts,
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
crate struct PipelineDesc {
}

#[derive(Debug)]
crate struct GraphicsPipeline {
    inner: vk::Pipeline,
    layout: Name,
    pass: Name,
    subpass: Name,
}

impl GraphicsPipeline {
    crate fn inner(&self) -> vk::Pipeline {
        self.inner
    }

    crate fn layout(&self) -> Name {
        self.layout
    }

    crate fn pass(&self) -> Name {
        self.pass
    }

    crate fn subpass(&self) -> Name {
        self.subpass
    }
}

pub(super) unsafe fn create_graphics_pipeline(
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
