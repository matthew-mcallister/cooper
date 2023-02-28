use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread;

use base::partial_map;
use engine::Engine;
use smallvec::smallvec;

#[derive(Debug)]
#[allow(dead_code)]
struct State {
    engine: Engine,
    tick: u64,
    graphics_queue: Arc<device::Queue>,
    transfer_queue: Arc<device::Queue>,
    render_passes: HashMap<String, Arc<device::RenderPass>>,
    attachments: Vec<Arc<device::ImageView>>,
    sw_index: u32,
    // Semaphore which presentation waits on
    backbuffer_semaphore: device::BinarySemaphore,
    // Single semaphore for keeping track of rendering completion
    frame_semaphore: device::TimelineSemaphore,
    // TODO: Thread-local command pools
    cmd_pool: Option<Box<device::CmdPool>>,
    cmd_buffer: vk::CommandBuffer,
}

impl State {
    fn new(engine: Engine) -> Self {
        let queue = Arc::clone(&engine.queues()[0][0]);
        Self {
            tick: 0,
            graphics_queue: Arc::clone(&queue),
            transfer_queue: queue,
            render_passes: create_render_passes(&engine),
            attachments: create_attachments(&engine),
            sw_index: 0,
            cmd_pool: Some(Box::new(
                engine.create_command_pool(0, vk::CommandPoolCreateFlags::TRANSIENT_BIT),
            )),
            cmd_buffer: vk::null(),
            backbuffer_semaphore: device::BinarySemaphore::new(engine.device_ref()),
            frame_semaphore: device::TimelineSemaphore::new(engine.device_ref(), 0),
            engine,
        }
    }

    fn render_pass(&self, s: &str) -> Arc<device::RenderPass> {
        Arc::clone(&self.render_passes[s])
    }

    fn swapchain_image(&self) -> &Arc<device::SwapchainView> {
        &self.engine.swapchain().views()[self.sw_index as usize]
    }
}

fn create_attachments(engine: &Engine) -> Vec<Arc<device::ImageView>> {
    let images = engine::create_framebuffer_images(
        engine,
        &[
            engine::FramebufferImageInfo {
                flags: device::ImageFlags::COLOR_ATTACHMENT | device::ImageFlags::INPUT_ATTACHMENT,
                format: device::Format::RGBA8,
                samples: device::SampleCount::One,
                name: Some("gbuffer"),
            },
            engine::FramebufferImageInfo {
                flags: device::ImageFlags::DEPTH_STENCIL_ATTACHMENT
                    | device::ImageFlags::INPUT_ATTACHMENT,
                format: device::Format::D24_S8,
                samples: device::SampleCount::One,
                name: Some("depth_stencil"),
            },
        ],
    );
    images
        .into_iter()
        .map(|img| img.create_full_view())
        .collect()
}

fn create_render_passes(engine: &Engine) -> HashMap<String, Arc<device::RenderPass>> {
    let mut map = HashMap::new();
    unsafe {
        map.insert(
            "main".to_owned(),
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
                // FIXME: Holy crap this is just a pointlessly different
                // version of the Vulkan API that adds nothing useful.
                vec![device::SubpassDesc {
                    layouts: vec![vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL],
                    color_attchs: vec![0],
                    ..Default::default()
                }],
                vec![vk::SubpassDependency {
                    src_subpass: vk::SUBPASS_EXTERNAL,
                    dst_subpass: 0,
                    src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT_BIT,
                    src_access_mask: Default::default(),
                    dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT_BIT,
                    dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE_BIT,
                    ..Default::default()
                }],
            ),
        );
    }
    map
}

fn record(state: &mut State) -> vk::CommandBuffer {
    let mut cmd_pool = state.cmd_pool.take().unwrap();
    if state.cmd_buffer == vk::null() {
        state.cmd_buffer = cmd_pool.alloc(device::CmdBufferLevel::Primary);
    }
    let mut cmds = unsafe {
        device::CmdBuffer::from_initial(cmd_pool, state.cmd_buffer, device::CmdBufferLevel::Primary)
    };
    draw_triangle(state, &mut cmds);
    let (cmds, pool) = cmds.end();
    state.cmd_pool = Some(pool);
    cmds
}

fn draw_triangle(state: &State, cmds: &mut device::CmdBuffer) {
    let engine = &state.engine;

    let render_pass = &state.render_pass("main");
    let attachments: [device::AttachmentImage; 1] = [
        Arc::clone(state.swapchain_image()).into(),
        //Arc::clone(&state.attachments[0]).into(),
        //Arc::clone(&state.attachments[1]).into(),
    ];
    engine.begin_render_pass(
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

    // TODO: Actually draw the triangle
    let vert_shader = Arc::clone(engine.get_shader("triangle_vert").unwrap());
    let frag_shader = Arc::clone(engine.get_shader("triangle_frag").unwrap());

    let pipelines = engine.pipelines();
    unsafe {
        let pipeline = pipelines.get_or_create_gfx(&device::GraphicsPipelineDesc {
            subpass: render_pass.subpass(0),
            layout: device::PipelineLayoutDesc {
                set_layouts: smallvec![],
            },
            vertex_layout: device::VertexInputLayout {
                topology: device::PrimitiveTopology::TriangleList,
                attributes: smallvec![],
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
        cmds.draw(3, 1);
    }
}

fn submit_commands(state: &mut State, commands: &[vk::CommandBuffer]) {
    unsafe {
        state.graphics_queue.submit(&[device::SubmitInfo {
            wait_sems: &[device::WaitInfo {
                semaphore: state.engine.acquire_semaphore_mut().inner_mut(),
                value: 0,
                stages: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT_BIT,
            }],
            sig_sems: &[
                device::SignalInfo {
                    semaphore: state.backbuffer_semaphore.inner_mut(),
                    value: 0,
                },
                device::SignalInfo {
                    semaphore: state.frame_semaphore.inner_mut(),
                    value: state.tick,
                },
            ],
            cmds: commands,
        }]);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Message {
    WindowClose,
}

fn main_loop(mut state: State, receiver: Receiver<Message>) {
    loop {
        state.frame_semaphore.wait(state.tick, 32_000_000).unwrap();
        state.tick += 1;
        state.engine.new_frame();
        unsafe {
            state.cmd_pool.as_mut().unwrap().reset();
        }
        state.sw_index = state.engine.acquire_next_image().unwrap();

        let cmds = record(&mut state);
        submit_commands(&mut state, &[cmds]);
        // TODO: Handle swapchain recreation
        unsafe {
            state.graphics_queue.present(
                &[&mut state.backbuffer_semaphore],
                state.engine.swapchain_mut(),
                state.sw_index,
            );
        }

        match receiver.try_recv() {
            Ok(Message::WindowClose) | Err(TryRecvError::Disconnected) => break,
            _ => {}
        }
    }
    state.engine.device().wait_idle();
}

fn main() {
    env_logger::init();

    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_inner_size(winit::dpi::LogicalSize {
            width: 1600,
            height: 900,
        })
        .build(&event_loop)
        .unwrap();

    let app_info = device::AppInfo {
        name: "deferred demo".into(),
        version: [0, 1, 0],
        debug: true,
        ..Default::default()
    };
    let mut engine = Engine::from_window(app_info, &window).unwrap();
    let path = Path::new(file!())
        .parent()
        .unwrap()
        .join("../../generated/shaders");
    engine.load_shaders_from_dir(&path).unwrap();

    let state = State::new(engine);
    let (sender, receiver) = channel();
    let mut j = Some(thread::spawn(|| main_loop(state, receiver)));

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_poll();
        match event {
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::CloseRequested,
                ..
            } => {
                let _ = sender.send(Message::WindowClose);
                let _ = j.take().unwrap().join();
                control_flow.set_exit();
            }
            _ => {}
        }
    });
}
