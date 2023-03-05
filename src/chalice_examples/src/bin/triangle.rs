//! An extremely minimal Vulkan application

use std::sync::Arc;

use base::partial_map;
use engine::Engine;
use smallvec::smallvec;
use tinker::Tinker;

#[derive(Debug)]
struct TriangleApp {
    render_pass: Arc<device::RenderPass>,
}

fn create_render_pass(engine: &Engine) -> Arc<device::RenderPass> {
    unsafe {
        device::RenderPass::new(
            engine.device_ref(),
            vec![device::AttachmentDescription {
                format: engine.swapchain().format(),
                samples: device::SampleCount::One,
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::STORE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                ..Default::default()
            }],
            vec![device::SubpassDesc::new(
                vec![vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL],
                vec![],
                vec![0],
                vec![],
                vec![],
                None,
            )],
            vec![vk::SubpassDependency {
                src_subpass: vk::SUBPASS_EXTERNAL,
                dst_subpass: 0,
                src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT_BIT,
                src_access_mask: Default::default(),
                dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT_BIT,
                dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE_BIT,
                ..Default::default()
            }],
        )
    }
}

fn record(app: &TriangleApp, tinker: &Tinker) -> vk::CommandBuffer {
    let render_pass = &app.render_pass;
    let level = vk::CommandBufferLevel::PRIMARY;
    let family = tinker.graphics_queue().family().index();
    tinker
        .engine()
        .with_command_buffer(level, family, |mut cmds| {
            cmds.begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT_BIT, None);
            begin_render_pass(tinker, render_pass, &mut cmds);
            draw_triangle(tinker.engine(), &mut cmds);
            cmds.end()
        })
}

fn begin_render_pass(
    tinker: &Tinker,
    render_pass: &Arc<device::RenderPass>,
    cmds: &mut device::CmdBuffer,
) {
    let attachments: [device::AttachmentImage; 1] =
        [Arc::clone(tinker.engine().swapchain_image()).into()];
    tinker.engine().begin_render_pass(
        cmds,
        render_pass,
        &attachments,
        // TODO: Write convenience functions for ClearColor
        &[vk::ClearValue {
            color: vk::ClearColorValue {
                float_32: [0.0, 0.0, 0.0, 0.0],
            },
        }],
    );
}

fn draw_triangle(engine: &Engine, cmds: &mut device::CmdBuffer) {
    let vert_shader = Arc::clone(engine.get_shader("triangle_vert").unwrap());
    let frag_shader = Arc::clone(engine.get_shader("triangle_frag").unwrap());

    let pipelines = engine.pipelines();
    unsafe {
        let pipeline = pipelines.get_or_create_gfx(&device::GraphicsPipelineDesc {
            subpass: cmds.subpass().unwrap(),
            layout: device::PipelineLayoutDesc {
                set_layouts: smallvec![],
            },
            vertex_layout: Default::default(),
            stages: partial_map! {
                device::ShaderStage::Vertex => Arc::new(vert_shader.into()),
                device::ShaderStage::Fragment => Arc::new(frag_shader.into()),
            },
            cull_mode: device::CullMode::None,
            wireframe: false,
            depth_test: false,
            depth_write: false,
            depth_cmp_op: Default::default(),
            depth_bias: false,
            blend_state: device::default_color_blend_state(),
            blend_consts: [0.0; 4],
        });
        cmds.bind_gfx_pipe(&pipeline);
        cmds.draw(3, 1);
    }
}

impl tinker::App for TriangleApp {
    fn app_info() -> device::AppInfo {
        device::AppInfo {
            name: "deferred demo".into(),
            version: [0, 1, 0],
            debug: true,
            ..Default::default()
        }
    }

    fn init(tinker: &mut Tinker) -> Self {
        TriangleApp {
            render_pass: create_render_pass(tinker.engine()),
        }
    }

    fn frame(&mut self, tinker: &mut Tinker) -> Vec<vk::CommandBuffer> {
        vec![record(self, tinker)]
    }
}

fn main() {
    tinker::run_app::<TriangleApp>(&chalice_examples::shader_dir());
}
