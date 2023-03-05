//! A simple Vulkan app that shows how to allocate and bind host-mapped
//! uniform and vertex buffers.

use std::sync::Arc;

use base::num::One;
use base::partial_map;
use device::RenderPass;
use engine::Engine;
use math::vec3;
use smallvec::smallvec;
use tinker::Tinker;

#[derive(Debug)]
struct CubeApp {
    render_pass: Arc<RenderPass>,
    index_buffer: device::BufferBox<[u32]>,
    vertex_buffer: device::BufferBox<[[f32; 3]]>,
    uniform_buffer: device::BufferBox<Uniforms>,
    descriptor_set: device::DescriptorSet,
}

fn create_render_pass(engine: &Engine) -> Arc<RenderPass> {
    unsafe {
        RenderPass::new(
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

const INDEX_DATA: &'static [u32] = &[
    0, 3, 1, //
    0, 2, 3, //
    4, 5, 7, //
    4, 7, 6, //
    1, 3, 7, //
    1, 7, 5, //
    0, 6, 2, //
    0, 4, 6, //
    2, 7, 3, //
    2, 6, 7, //
    0, 1, 5, //
    0, 5, 4, //
];

const VERTEX_DATA: &'static [[f32; 3]] = &[
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [1.0, -1.0, 1.0],
    [-1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0],
];

#[derive(Clone, Copy, Debug, Default)]
struct Uniforms {
    projection: math::Matrix4,
    time: f32,
}

// Creates a memory mapped(!) index and vertex buffer, as well as a
// uniform buffer.
fn create_buffers(
    engine: &Engine,
) -> (
    device::BufferBox<[u32]>,
    device::BufferBox<[[f32; 3]]>,
    device::BufferBox<Uniforms>,
) {
    let index_alloc = engine.buffer_heap().alloc(
        device::BufferBinding::Index,
        device::Lifetime::Static,
        device::MemoryMapping::Mapped,
        (INDEX_DATA.len() * std::mem::size_of::<u32>()) as vk::DeviceSize,
    );
    let index_buffer = device::BufferBox::copy_from_slice(index_alloc, INDEX_DATA);
    let vertex_alloc = engine.buffer_heap().alloc(
        device::BufferBinding::Vertex,
        device::Lifetime::Static,
        device::MemoryMapping::Mapped,
        (VERTEX_DATA.len() * std::mem::size_of::<[f32; 3]>()) as vk::DeviceSize,
    );
    let vertex_buffer = device::BufferBox::copy_from_slice(vertex_alloc, VERTEX_DATA);
    let uniform_alloc = engine.buffer_heap().alloc(
        device::BufferBinding::Uniform,
        device::Lifetime::Static,
        device::MemoryMapping::Mapped,
        (VERTEX_DATA.len() * std::mem::size_of::<[f32; 3]>()) as vk::DeviceSize,
    );
    let uniform_buffer = device::BufferBox::from_val(uniform_alloc, Default::default());
    (index_buffer, vertex_buffer, uniform_buffer)
}

fn create_descriptor_set(
    engine: &Engine,
    uniforms: &device::BufferBox<Uniforms>,
) -> device::DescriptorSet {
    let desc = device::DescriptorSetLayoutDesc {
        bindings: smallvec![device::DescriptorSetLayoutBinding {
            binding: 0,
            ty: device::DescriptorType::UniformBuffer,
            ..Default::default()
        }],
    };
    let layout = Arc::new(device::DescriptorSetLayout::new(engine.device_ref(), desc));
    let mut set = engine
        .descriptor_heap()
        .alloc(device::Lifetime::Static, &layout);
    set.write_buffer(0, device::BufferBox::range(uniforms));
    set
}

fn update_uniforms(tinker: &Tinker, uniforms: &mut device::BufferBox<Uniforms>) {
    let pos = vec3(0.0, 0.0, -3.0);
    let axis1 = vec3(-1.0, 2.0, -1.0).normalized();
    let angle1 = tinker.elapsed_time() * std::f32::consts::FRAC_PI_6 * 1.11;
    let rot1 = math::Quaternion::from_axis_angle(axis1, angle1).to_mat4();
    let axis2 = vec3(1.0, 1.0, 3.0).normalized();
    let angle2 = tinker.elapsed_time() * std::f32::consts::FRAC_PI_3 * 0.93;
    let rot2 = math::Quaternion::from_axis_angle(axis2, angle2).to_mat4();
    let model = rot1 * rot2;
    let view: math::Matrix3 = One::one();
    let view = view.translate(-pos);
    let proj = tinker.perspective(0.01, 100.0, 45.0);
    uniforms.projection = proj * view * model;
    uniforms.time = tinker.elapsed_time();
}

fn record(app: &CubeApp, tinker: &Tinker) -> vk::CommandBuffer {
    let level = vk::CommandBufferLevel::PRIMARY;
    let family = tinker.graphics_queue().family().index();
    tinker
        .engine()
        .with_command_buffer(level, family, |mut cmds| {
            cmds.begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT_BIT, None);
            begin_render_pass(tinker, &app.render_pass, &mut cmds);
            draw_cube(app, tinker.engine(), &mut cmds);
            cmds.end()
        })
}

fn begin_render_pass(tinker: &Tinker, render_pass: &Arc<RenderPass>, cmds: &mut device::CmdBuffer) {
    let attachments: [device::AttachmentImage; 1] =
        [Arc::clone(tinker.engine().swapchain_image()).into()];
    tinker.engine().begin_render_pass(
        cmds,
        render_pass,
        &attachments,
        &[vk::ClearValue {
            color: vk::ClearColorValue {
                float_32: [0.0, 0.0, 0.0, 0.0],
            },
        }],
    );
}

fn draw_cube(app: &CubeApp, engine: &Engine, cmds: &mut device::CmdBuffer) {
    let vert_shader = Arc::clone(engine.get_shader("cube_vert").unwrap());
    let frag_shader = Arc::clone(engine.get_shader("cube_frag").unwrap());

    cmds.bind_index_buffer(
        device::BufferBox::range(&app.index_buffer),
        device::IndexType::U32,
    );
    cmds.bind_vertex_buffers([device::BufferBox::range(&app.vertex_buffer)]);

    let pipelines = engine.pipelines();
    unsafe {
        let pipeline = pipelines.get_or_create_gfx(&device::GraphicsPipelineDesc {
            subpass: cmds.subpass().unwrap(),
            layout: device::PipelineLayoutDesc {
                set_layouts: smallvec![Arc::clone(app.descriptor_set.layout())],
            },
            vertex_layout: device::VertexInputLayout {
                topology: device::PrimitiveTopology::TriangleList,
                bindings: smallvec![vk::VertexInputBindingDescription {
                    binding: 0,
                    stride: std::mem::size_of::<[f32; 3]>() as _,
                    input_rate: vk::VertexInputRate::VERTEX,
                }],
                attributes: smallvec![vk::VertexInputAttributeDescription {
                    location: 0,
                    binding: 0,
                    format: vk::Format::R32G32B32_SFLOAT,
                    offset: 0,
                }],
            },
            stages: partial_map! {
                device::ShaderStage::Vertex => Arc::new(vert_shader.into()),
                device::ShaderStage::Fragment => Arc::new(frag_shader.into()),
            },
            cull_mode: device::CullMode::Back,
            wireframe: false,
            depth_test: false,
            depth_write: false,
            depth_cmp_op: Default::default(),
            depth_bias: false,
            blend_state: device::default_color_blend_state(),
            blend_consts: [0.0; 4],
        });
        cmds.bind_gfx_pipe(&pipeline);
        cmds.bind_gfx_descs(0, &app.descriptor_set);
        cmds.draw_indexed(INDEX_DATA.len() as _, 1);
    }
}

impl tinker::App for CubeApp {
    fn app_info() -> device::AppInfo {
        device::AppInfo {
            name: "deferred demo".into(),
            version: [0, 1, 0],
            debug: true,
            ..Default::default()
        }
    }

    fn init(tinker: &mut Tinker) -> Self {
        let (idx, vtx, uniform) = create_buffers(tinker.engine());
        CubeApp {
            render_pass: create_render_pass(tinker.engine()),
            descriptor_set: create_descriptor_set(tinker.engine(), &uniform),
            index_buffer: idx,
            vertex_buffer: vtx,
            uniform_buffer: uniform,
        }
    }

    fn frame(&mut self, tinker: &mut Tinker) -> Vec<vk::CommandBuffer> {
        update_uniforms(tinker, &mut self.uniform_buffer);
        vec![record(self, tinker)]
    }
}

fn main() {
    tinker::run_app::<CubeApp>(&chalice_examples::shader_dir());
}
