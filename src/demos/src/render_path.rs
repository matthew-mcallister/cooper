use std::sync::Arc;

use crate::*;

const VERT_SHADER_SRC: &'static [u8] = include_shader!("triangle_vert.spv");
const FRAG_SHADER_SRC: &'static [u8] = include_shader!("triangle_frag.spv");

#[derive(Clone, Copy, Debug)]
pub struct Framebuffer {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub inner: vk::Framebuffer,
}

#[derive(Debug)]
pub struct RenderPath {
    pub swapchain: Arc<Swapchain>,
    pub objs: Box<ObjectTracker>,
    pub framebuffers: Vec<Framebuffer>,
    pub render_pass: vk::RenderPass,
    pub pipeline: vk::Pipeline,
}

impl RenderPath {
    pub unsafe fn new(swapchain: Arc<Swapchain>) -> RenderPath {
        init_render_path(swapchain)
    }
}

unsafe fn init_render_path(swapchain: Arc<Swapchain>) -> RenderPath {
    let mut objs = Box::new(ObjectTracker::new(Arc::clone(&swapchain.device)));

    let attachments = [vk::AttachmentDescription {
        format: swapchain.format,
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
    let create_info = vk::RenderPassCreateInfo {
        attachment_count: attachments.len() as _,
        p_attachments: attachments.as_ptr(),
        subpass_count: subpasses.len() as _,
        p_subpasses: subpasses.as_ptr(),
        ..Default::default()
    };
    let render_pass = objs.create_render_pass(&create_info);

    let create_info = Default::default();
    let layout = objs.create_pipeline_layout(&create_info);

    let vert_shader = objs.create_shader(VERT_SHADER_SRC);
    let frag_shader = objs.create_shader(FRAG_SHADER_SRC);

    let p_name = c_str!("main");
    let stages = [
        vk::PipelineShaderStageCreateInfo {
            stage: vk::ShaderStageFlags::VERTEX_BIT,
            module: vert_shader,
            p_name,
            ..Default::default()
        },
        vk::PipelineShaderStageCreateInfo {
            stage: vk::ShaderStageFlags::FRAGMENT_BIT,
            module: frag_shader,
            p_name,
            ..Default::default()
        },
    ];
    let vertex_input_state = Default::default();
    let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo {
        topology: vk::PrimitiveTopology::TRIANGLE_STRIP,
        ..Default::default()
    };
    let viewports = [vk::Viewport {
        x: 0.0,
        y: 0.0,
        width: swapchain.extent.width as _,
        height: swapchain.extent.height as _,
        min_depth: 0.0,
        max_depth: 1.0,
    }];
    let scissors = [swapchain.rectangle()];
    let viewport_state = vk::PipelineViewportStateCreateInfo {
        viewport_count: viewports.len() as _,
        p_viewports: viewports.as_ptr(),
        scissor_count: scissors.len() as _,
        p_scissors: scissors.as_ptr(),
        ..Default::default()
    };
    let rasterization_state = vk::PipelineRasterizationStateCreateInfo {
        polygon_mode: vk::PolygonMode::FILL,
        line_width: 1.0,
        ..Default::default()
    };
    let multisample_state = vk::PipelineMultisampleStateCreateInfo {
        rasterization_samples: vk::SampleCountFlags::_1_BIT,
        ..Default::default()
    };
    let color_blend_attachments = [vk::PipelineColorBlendAttachmentState {
        color_write_mask: vk::ColorComponentFlags::R_BIT
            | vk::ColorComponentFlags::G_BIT
            | vk::ColorComponentFlags::B_BIT
            | vk::ColorComponentFlags::A_BIT,
        ..Default::default()
    }];
    let color_blend_state = vk::PipelineColorBlendStateCreateInfo {
        attachment_count: color_blend_attachments.len() as _,
        p_attachments: color_blend_attachments.as_ptr(),
        ..Default::default()
    };
    let create_info = vk::GraphicsPipelineCreateInfo {
        stage_count: stages.len() as _,
        p_stages: stages.as_ptr(),
        p_vertex_input_state: &vertex_input_state as _,
        p_input_assembly_state: &input_assembly_state as _,
        p_viewport_state: &viewport_state as _,
        p_rasterization_state: &rasterization_state as _,
        p_multisample_state: &multisample_state as _,
        p_color_blend_state: &color_blend_state as _,
        layout,
        render_pass,
        subpass: 0,
        ..Default::default()
    };
    let pipeline = objs.create_graphics_pipeline(&create_info);

    let framebuffers: Vec<_> = swapchain.images.iter().map(|&image| {
        let create_info = vk::ImageViewCreateInfo {
            image,
            view_type: vk::ImageViewType::_2D,
            format: swapchain.format,
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR_BIT,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            },
            ..Default::default()
        };
        let view = objs.create_image_view(&create_info as _);

        let attachments = std::slice::from_ref(&view);
        let create_info = vk::FramebufferCreateInfo {
            render_pass,
            attachment_count: attachments.len() as _,
            p_attachments: attachments.as_ptr(),
            width: swapchain.extent.width,
            height: swapchain.extent.height,
            layers: 1,
            ..Default::default()
        };
        let inner = objs.create_framebuffer(&create_info as _);

        Framebuffer {
            image,
            view,
            inner,
        }
    }).collect();

    RenderPath {
        swapchain,
        objs,
        framebuffers,
        render_pass,
        pipeline,
    }
}
