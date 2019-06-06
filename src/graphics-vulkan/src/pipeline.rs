#![allow(dead_code)]
use std::collections::HashMap;
use std::ptr;
use std::sync::Arc;

use alloc::{Allocator, BumpAllocator};
use memoffset::offset_of;

use crate::*;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
crate enum PipelineGeometry {
    Rigid,
    Jointed,
    //Deforming,
}

impl PipelineGeometry {
    crate fn vertex_attrs(self) -> VertexAttrs {
        match self {
            PipelineGeometry::Static => VertexAttrs::PosNormTex0Tan,
            PipelineGeometry::Jointed => VertexAttrs::PosNormTex0TanJointss,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
crate enum AlphaMode {
    //Blend,
    //ToCoverage,
    //... or alpha test or whatever
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
crate enum PipelineOutput {
    Depth,
    Lighting {
        // TODO: Switch to a string wrapper (around say [u8; 31]).
        frag_shader: &'static str,
        alpha_mode: Option<AlphaMode>,
    },
}

/// Describes a pipeline in a way that a pipeline manager can fetch a
/// matching pre-existing pipeline.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
crate struct PipelineDesc {
    crate geometry: PipelineGeometry,
    crate output: PipelineOutput,
}

fn vertex_size(attrs: VertexAttrs) -> usize {
    match attrs {
        VertexAttrs::PosNormTex0Tan => std::mem::size_of::<VertexPnxt>(),
        VertexAttrs::PosNormTex0TanJoints =>
            std::mem::size_of::<VertexPnxtj>(),
    }
}

fn vertex_attr_desc(attrs: VertexAttrs) ->
    Vec<vk::VertexInputAttributeDescription>
{
    let pos = vk::VertexInputAttributeDescription {
        binding: 0,
        location: 0,
        format: vk::Format::R32G32B32_SFLOAT,
        offset: offset_of!(VertexPnxt, position) as _,
    };
    let normal = vk::VertexInputAttributeDescription {
        binding: 0,
        location: 1,
        format: vk::Format::R32G32B32_SFLOAT,
        offset: offset_of!(VertexPnxt, normal) as _,
    };
    let tex_coord_0 = vk::VertexInputAttributeDescription {
        binding: 0,
        location: 2,
        format: vk::Format::R32G32_SFLOAT,
        offset: offset_of!(VertexPnxt, tex_coord_0) as _,
    };
    let tan = vk::VertexInputAttributeDescription {
        binding: 0,
        location: 3,
        format: vk::Format::R32G32B32_SFLOAT,
        offset: offset_of!(VertexPnxt, tangent) as _,
    };
    let joint_index = vk::VertexInputAttributeDescription {
        binding: 0,
        location: 4,
        format: vk::Format::R8G8B8A8_UINT,
        offset: offset_of!(VertexPnxtj, joints) as _,
    };
    let joint_weight = vk::VertexInputAttributeDescription {
        binding: 0,
        location: 5,
        format: vk::Format::R8G8B8A8_UNORM,
        offset: offset_of!(VertexPnxtj, weights) as _,
    };

    match attrs {
        VertexAttrs::PosNormTex0Tan =>
            vec![pos, normal, tex_coord_0, tan],
        VertexAttrs::PosNormTex0TanJoints =>
            vec![pos, normal, tex_coord_0, tan, joint_index, joint_weight],
    }
}

#[derive(Debug)]
crate struct PipelineMap {
    dt: Arc<vkl::DeviceTable>,
    pipelines: HashMap<PipelineDesc, vk::Pipeline>,
}

impl Drop for PipelineMap {
    fn drop(&mut self) {
        for &pipe in self.pipelines.values() {
            unsafe {
                self.dt.destroy_pipeline(pipe, ptr::null());
            }
        }
    }
}

impl PipelineMap {
    fn get(&self, desc: &PipelineDesc) -> vk::Pipeline {
        self.pipelines[desc]
    }
}

fn validate_shader_usage(shader: &ShaderObj, layout: &PipelineLayoutObj) {
    assert!(shader.set_layouts.iter().cloned()
        .all(|(idx, set)| layout.set_layouts[idx as usize] == set))
}

struct PipelineBuilder<'a> {
    swapchain: &'a Swapchain,
    render_passes: &'a HashCollection<RenderPassObj>,
    shaders: &'a HashCollection<ShaderObj>,
    layouts: &'a HashCollection<PipelineLayoutObj>,
    allocator: BumpAllocator,
    create_info: Vec<vk::GraphicsPipelineCreateInfo>,
}

unsafe fn add_pipeline(builder: &mut PipelineBuilder<'_>, desc: &PipelineDesc)
{
    let allocator = &mut builder.allocator;

    // Step 1: render pass, shader stages, pipeline layout

    let render_pass = builder.render_passes.get("forward");

    // TODO: frag shader can be chosen freely, but vertex/tess shaders
    // determined by choice of frag shader
    assert_eq!(desc.geometry, PipelineGeometry::Static);
    let subpass;
    let sh_stages: &'static [&'static str];
    match desc.output {
        PipelineOutput::Depth => {
            subpass = "depth";
            sh_stages = &["static_depth_vert"];
        },
        PipelineOutput::Lighting { frag_shader, .. } => {
            assert_eq!(frag_shader, "pbr_frag");
            subpass = "lighting";
            sh_stages = &["static_pbr_vert", "pbr_frag"];
        },
    }
    let subpass = render_pass.subpasses[subpass];
    let layout = builder.layouts.get("pbr");

    let mut stages = Vec::new();
    for stage in sh_stages.iter() {
        let shader = builder.shaders.get(stage);
        validate_shader_usage(shader, layout);
        stages.push(vk::PipelineShaderStageCreateInfo {
            module: shader.obj,
            stage: shader.stage,
            p_name: c_str!("main"),
            ..Default::default()
        });
    }
    let stages = allocator.alloc_slice(&stages);

    // Step 2: geometry config

    // TODO: If position is stored in a separate buffer, improved cache
    // coherency during the depth-only pass may help performance.
    let attrs = desc.geometry.vertex_attrs();
    let vertex_bindings = allocator.alloc_slice(&[
        vk::VertexInputBindingDescription {
            binding: 0,
            stride: vertex_size(attrs) as _,
            input_rate: vk::VertexInputRate::VERTEX,
        }
    ]);
    let vertex_attrs = allocator.alloc_slice(&vertex_attr_desc(attrs));
    let p_vertex_input_state = allocator.alloc_val(
        vk::PipelineVertexInputStateCreateInfo {
            vertex_binding_description_count: (*vertex_bindings).len() as _,
            p_vertex_binding_descriptions: vertex_bindings as _,
            vertex_attribute_description_count: (*vertex_attrs).len() as _,
            p_vertex_attribute_descriptions: vertex_attrs as _,
            ..Default::default()
        }
    );
    let p_input_assembly_state = allocator.alloc_val(
        vk::PipelineInputAssemblyStateCreateInfo {
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            ..Default::default()
        }
    );

    // Step 3: output config

    let swapchain = &builder.swapchain;
    let extent = swapchain.create_info.image_extent;
    let viewports = allocator.alloc_slice(&[vk::Viewport {
        x: 0.0,
        y: 0.0,
        width: extent.width as _,
        height: extent.height as _,
        min_depth: 0.0,
        max_depth: 1.0,
    }]);
    let scissors = allocator.alloc_slice(&[vk::Rect2D {
        offset: vk::Offset2D::new(0, 0),
        extent,
    }]);
    let p_viewport_state = allocator.alloc_val(
        vk::PipelineViewportStateCreateInfo {
            viewport_count: (*viewports).len() as _,
            p_viewports: viewports as _,
            scissor_count: (*scissors).len() as _,
            p_scissors: scissors as _,
            ..Default::default()
        }
    );
    let p_multisample_state = allocator.alloc_val(
        vk::PipelineMultisampleStateCreateInfo {
            rasterization_samples: vk::SampleCountFlags::_1_BIT,
            ..Default::default()
        }
    );

    let output = &desc.output;
    let (depth_write_enable, rasterizer_discard_enable);
    match output {
        PipelineOutput::Depth => {
            depth_write_enable = vk::TRUE;
            rasterizer_discard_enable = vk::TRUE;
        },
        PipelineOutput::Lighting { alpha_mode, .. } => {
            depth_write_enable = bool32(alpha_mode.is_some());
            rasterizer_discard_enable = vk::FALSE;
        },
    }

    let p_depth_stencil_state = allocator.alloc_val(
        vk::PipelineDepthStencilStateCreateInfo {
            depth_test_enable: vk::TRUE,
            depth_write_enable,
            depth_compare_op: vk::CompareOp::LESS,
            ..Default::default()
        }
    );
    let p_rasterization_state = allocator.alloc_val(
        vk::PipelineRasterizationStateCreateInfo {
            rasterizer_discard_enable,
            cull_mode: vk::CullModeFlags::FRONT_BIT,
            line_width: 1.0,
            ..Default::default()
        }
    );
    let p_color_blend_state = match output {
        PipelineOutput::Depth => ptr::null(),
        PipelineOutput::Lighting { alpha_mode, .. } => {
            assert!(alpha_mode.is_none()); // TODO
            let attachments: *mut [vk::PipelineColorBlendAttachmentState] =
                allocator.alloc_slice(&[Default::default()]);
            allocator.alloc_val(vk::PipelineColorBlendStateCreateInfo {
                attachment_count: (*attachments).len() as _,
                p_attachments: attachments as _,
                ..Default::default()
            })
        }
    };

    builder.create_info.push(vk::GraphicsPipelineCreateInfo {
        stage_count: (*stages).len() as _,
        p_stages: stages as _,
        p_vertex_input_state,
        p_input_assembly_state,
        p_viewport_state,
        p_rasterization_state,
        p_multisample_state,
        p_depth_stencil_state,
        p_color_blend_state,
        layout: layout.obj,
        render_pass: render_pass.obj,
        subpass,
        ..Default::default()
    });
}

// TODO: Just generate these on-demand and cache the results.
unsafe fn build_pipelines(builder: &PipelineBuilder<'_>) ->
    Result<Vec<vk::Pipeline>, vk::Result>
{
    let create_info = &builder.create_info;
    let mut pipelines = Vec::with_capacity(create_info.len());
    builder.swapchain.dt.create_graphics_pipelines(
        vk::null(),
        create_info.len() as _,
        create_info.as_ptr(),
        ptr::null(),
        pipelines.as_mut_ptr(),
    ).check()?;
    pipelines.set_len(create_info.len());
    Ok(pipelines)
}

fn pipeline_descs() -> &'static [PipelineDesc] {
    &[
        PipelineDesc {
            geometry: PipelineGeometry::Static,
            output: PipelineOutput::Depth,
        },
        PipelineDesc {
            geometry: PipelineGeometry::Static,
            output: PipelineOutput::Lighting {
                frag_shader: "pbr_frag",
                alpha_mode: None,
            },
        },
    ]
}

crate unsafe fn create_pipeline_map(
    swapchain: &Swapchain,
    render_passes: &HashCollection<RenderPassObj>,
    shaders: &HashCollection<ShaderObj>,
    layouts: &HashCollection<PipelineLayoutObj>,
) -> Result<PipelineMap, vk::Result> {
    let descs = pipeline_descs();
    let mut builder = PipelineBuilder {
        swapchain, render_passes, shaders, layouts,
        allocator: BumpAllocator::with_capacity(0x8000),
        create_info: Vec::new(),
    };
    for desc in descs { add_pipeline(&mut builder, desc); }
    let pipelines = build_pipelines(&builder)?;
    let map: HashMap<_, _> = descs.iter().cloned()
        .zip(pipelines.iter().cloned())
        .collect();
    Ok(PipelineMap { dt: Arc::clone(&swapchain.dt), pipelines: map })
}
