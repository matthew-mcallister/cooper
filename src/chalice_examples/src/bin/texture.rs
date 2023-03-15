//! Demonstrates how to stage, upload, and bind sampled images

use std::sync::Arc;

use base::num::One;
use base::partial_map;
use device::RenderPass;
use engine::Engine;
use math::vec3;
use smallvec::smallvec;
use tinker::Tinker;

#[derive(Debug)]
struct TextureApp {
    image_data: image::RgbaImage,
    render_pass: Arc<RenderPass>,
    index_buffer: device::BufferAlloc,
    vertex_buffer: device::BufferAlloc,
    uniform_buffer: device::BufferBox<Uniforms>,
    texture: Arc<device::ImageView>,
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
    0, 1, 2, //
    0, 2, 3, //
];

type Vertex = ([f32; 3], [f32; 2]);

const VERTEX_DATA: &'static [Vertex] = &[
    ([-1.0, 1.0, 0.0], [0.0, 1.0]),
    ([1.0, 1.0, 0.0], [1.0, 1.0]),
    ([1.0, -1.0, 0.0], [1.0, 0.0]),
    ([-1.0, -1.0, 0.0], [0.0, 0.0]),
];

#[derive(Clone, Copy, Debug, Default)]
struct Uniforms {
    projection: math::Matrix4,
}

fn create_resources(
    engine: &Engine,
    image: &image::RgbaImage,
) -> (
    device::BufferAlloc,
    device::BufferAlloc,
    device::BufferBox<Uniforms>,
    Arc<device::ImageView>,
) {
    let index_alloc = engine.buffer_heap().alloc(
        device::BufferBinding::Index,
        device::Lifetime::Static,
        device::MemoryMapping::DeviceLocal,
        (INDEX_DATA.len() * std::mem::size_of::<u32>()) as vk::DeviceSize,
    );
    let vertex_alloc = engine.buffer_heap().alloc(
        device::BufferBinding::Vertex,
        device::Lifetime::Static,
        device::MemoryMapping::DeviceLocal,
        (VERTEX_DATA.len() * std::mem::size_of::<Vertex>()) as vk::DeviceSize,
    );
    let uniform_alloc = engine.buffer_heap().alloc(
        device::BufferBinding::Uniform,
        device::Lifetime::Static,
        device::MemoryMapping::Mapped,
        std::mem::size_of::<Uniforms>() as vk::DeviceSize,
    );
    let uniform_buffer = device::BufferBox::from_val(uniform_alloc, Default::default());
    let texture = Arc::new(device::Image::new(
        engine.image_heap(),
        device::ImageDef::new(
            engine.device(),
            Default::default(),
            device::ImageType::Dim2,
            device::Format::RGBA8,
            device::SampleCount::One,
            device::Extent3D::new(image.width(), image.height(), 1),
            1,
            1,
        )
        .with_name("square_texture".to_owned())
        .into(),
    ));
    let texture = texture.create_full_view();
    (index_alloc, vertex_alloc, uniform_buffer, texture)
}

fn create_descriptor_set(
    engine: &Engine,
    uniforms: &device::BufferBox<Uniforms>,
    image: &device::ImageView,
) -> device::DescriptorSet {
    let sampler = engine.samplers().get_or_create(&device::SamplerDesc {
        mag_filter: device::Filter::Linear,
        min_filter: device::Filter::Linear,
        mipmap_mode: device::SamplerMipmapMode::Linear,
        address_mode_u: device::SamplerAddressMode::Repeat,
        address_mode_v: device::SamplerAddressMode::Repeat,
        address_mode_w: device::SamplerAddressMode::Repeat,
        anisotropy_level: device::AnisotropyLevel::Sixteen,
        ..Default::default()
    });
    engine.create_descriptor_set(
        device::Lifetime::Static,
        Some("globals"),
        &[
            engine::DescriptorResource::UniformBuffers(
                &[device::BufferBox::range(uniforms)],
                vk::ShaderStageFlags::ALL,
            ),
            engine::DescriptorResource::ImageSamplers {
                images: &[(image, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)],
                samplers: &[&sampler],
                stage_flags: vk::ShaderStageFlags::FRAGMENT_BIT,
                immutable_samplers: true,
            },
        ],
    )
}

fn upload_data(tinker: &Tinker, app: &mut TextureApp) {
    let level = vk::CommandBufferLevel::PRIMARY;
    let family = tinker.transfer_queue().family().index();
    let engine = tinker.engine();
    engine.with_command_buffer(level, family, |mut cmds| {
        let mut staging = engine.staging().lock().unwrap();
        cmds.begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT_BIT, None);
        staging.stage_image(
            &mut cmds,
            &app.image_data,
            app.texture.image(),
            Default::default(),
        );
        staging.stage_buffer(
            &mut cmds,
            base::slice_to_bytes(INDEX_DATA),
            &mut app.index_buffer.range(),
            Default::default(),
        );
        staging.stage_buffer(
            &mut cmds,
            base::slice_to_bytes(VERTEX_DATA),
            &mut app.vertex_buffer.range(),
            Default::default(),
        );
        staging.submit(cmds);
        staging.wait(100_000_000).unwrap();
    });
}

fn update_uniforms(tinker: &Tinker, uniforms: &mut device::BufferBox<Uniforms>) {
    let axis1 = vec3(0.0, 0.0, 1.0);
    let s1 = tinker.elapsed_time() * std::f32::consts::FRAC_PI_6 * 1.11;
    let angle1 = std::f32::consts::PI * s1.sin() / 32.0;
    let rot1 = math::Quaternion::from_axis_angle(axis1, angle1).to_mat4();
    let axis2 = vec3(0.0, 1.0, 0.0).normalized();
    let s2 = tinker.elapsed_time() * std::f32::consts::FRAC_PI_3 * 0.93;
    let angle2 = std::f32::consts::PI * s2.sin() / 16.0;
    let rot2 = math::Quaternion::from_axis_angle(axis2, angle2).to_mat4();
    let model = rot1 * rot2;

    let pos = vec3(0.0, 0.0, -3.0);
    let view: math::Matrix3 = One::one();
    let view = view.translate(-pos);

    let proj = tinker.perspective(0.01, 100.0, 45.0);

    uniforms.projection = proj * view * model;
}

fn record(app: &TextureApp, tinker: &Tinker) -> vk::CommandBuffer {
    let level = vk::CommandBufferLevel::PRIMARY;
    let family = tinker.graphics_queue().family().index();
    tinker
        .engine()
        .with_command_buffer(level, family, |mut cmds| {
            cmds.begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT_BIT, None);
            begin_render_pass(tinker, &app.render_pass, &mut cmds);
            draw_thing(app, tinker.engine(), &mut cmds);
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

fn draw_thing(app: &TextureApp, engine: &Engine, cmds: &mut device::CmdBuffer) {
    let vert_shader = Arc::clone(engine.get_shader("texture_vert").unwrap());
    let frag_shader = Arc::clone(engine.get_shader("texture_frag").unwrap());

    cmds.bind_index_buffer(app.index_buffer.range(), device::IndexType::U32);
    cmds.bind_vertex_buffers([app.vertex_buffer.range()]);

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
                    stride: std::mem::size_of::<Vertex>() as _,
                    input_rate: vk::VertexInputRate::VERTEX,
                }],
                attributes: smallvec![
                    vk::VertexInputAttributeDescription {
                        location: 0,
                        binding: 0,
                        format: vk::Format::R32G32B32_SFLOAT,
                        offset: 0,
                    },
                    vk::VertexInputAttributeDescription {
                        location: 1,
                        binding: 0,
                        format: vk::Format::R32G32_SFLOAT,
                        offset: 12,
                    }
                ],
            },
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
        cmds.bind_gfx_descs(0, &app.descriptor_set);
        cmds.draw_indexed(INDEX_DATA.len() as _, 1);
    }
}

impl tinker::App for TextureApp {
    fn app_info() -> device::AppInfo {
        device::AppInfo {
            name: "texture demo".into(),
            version: [0, 1, 0],
            debug: true,
            ..Default::default()
        }
    }

    fn init(tinker: &mut Tinker) -> Self {
        let path = std::env::var("IMAGE_PATH").unwrap();
        let image_data = image::open(path).unwrap().to_rgba8();
        let (idx, vtx, uniform, texture) = create_resources(tinker.engine(), &image_data);
        let mut app = TextureApp {
            image_data,
            render_pass: create_render_pass(tinker.engine()),
            descriptor_set: create_descriptor_set(tinker.engine(), &uniform, &texture),
            index_buffer: idx,
            vertex_buffer: vtx,
            texture,
            uniform_buffer: uniform,
        };
        upload_data(tinker, &mut app);
        app
    }

    fn frame(&mut self, tinker: &mut Tinker) -> Vec<vk::CommandBuffer> {
        update_uniforms(tinker, &mut self.uniform_buffer);
        vec![record(self, tinker)]
    }
}

fn main() {
    tinker::run_app::<TextureApp>(&chalice_examples::shader_dir());
}
