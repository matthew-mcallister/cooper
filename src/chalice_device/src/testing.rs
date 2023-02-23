use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use winit::window::Window;

use crate::*;

const WINDOW_NAME: &str = "cooper test";
const WINDOW_DIMS: (u32, u32) = (1920, 1080);

fn app_info() -> AppInfo {
    AppInfo {
        name: WINDOW_NAME.to_owned(),
        version: [0, 1, 0],
        debug: true,
        test: true,
        ..Default::default()
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct TestVars {
    pub(crate) window: Window,
    pub(crate) swapchain: Swapchain,
    pub(crate) queues: Vec<Vec<Arc<Queue>>>,
}

fn create_window_inner(event_loop: &winit::event_loop::EventLoop<()>) -> Window {
    winit::window::WindowBuilder::new()
        .with_inner_size(winit::dpi::Size::Physical(winit::dpi::PhysicalSize {
            width: WINDOW_DIMS.0,
            height: WINDOW_DIMS.1,
        }))
        .with_title(WINDOW_NAME)
        .with_visible(false)
        .build(&event_loop)
        .map_err(|_| "Failed to create window")
        .unwrap()
}

lazy_static::lazy_static! {
    // Yes, we seriously have to do this all because winit relies on
    // thread-local mutable state, *even on Linux*!
    static ref CHANNEL: Mutex<(Sender<()>, Receiver<Window>)> = {
        use winit::platform::x11::EventLoopBuilderExtX11;
        let (send, foreign_recv) = std::sync::mpsc::channel::<()>();
        let (foreign_send, recv) = std::sync::mpsc::channel::<Window>();
        std::mem::forget(std::thread::spawn(move || {
            let event_loop = winit::event_loop::EventLoopBuilder::new()
                .with_any_thread(true)
                .build();
            loop {
                foreign_recv.recv().unwrap();
                foreign_send.send(create_window_inner(&event_loop)).unwrap();
            }
        }));
        Mutex::new((send, recv))
    };
}

fn create_window() -> Window {
    let channel = CHANNEL.lock().unwrap();
    channel.0.send(()).unwrap();
    channel.1.recv().unwrap()
}

static INIT_LOGGING: std::sync::Once = std::sync::Once::new();

impl TestVars {
    pub(crate) fn new() -> Self {
        INIT_LOGGING.call_once(env_logger::init);
        let window = create_window();
        let (swapchain, queues) = init_device_and_swapchain(app_info(), &window).unwrap();
        TestVars {
            window,
            swapchain,
            queues,
        }
    }

    pub(crate) fn swapchain(&self) -> &Swapchain {
        &self.swapchain
    }

    pub(crate) fn device(&self) -> &Arc<Device> {
        &self.swapchain.device
    }

    pub(crate) fn gfx_queue(&self) -> &Arc<Queue> {
        &self.queues[0][0]
    }
}

macro_rules! test_shaders {
    ($($name:ident,)*) => {
        #[derive(Debug)]
        pub(crate) struct TestShaders {
            $(pub(crate) $name: Arc<Shader>,)*
        }

        impl TestShaders {
            pub(crate) fn new(device: &Arc<Device>) -> Self {
                unsafe {
                    TestShaders {
                        $($name: Arc::new(Shader::from_path(
                            Arc::clone(device),
                            concat!(
                                env!("CARGO_MANIFEST_DIR"),
                                "/data/", stringify!($name), ".spv",
                            ),
                        ).unwrap()),)*
                    }
                }
            }
        }
    }
}
test_shaders! {
    trivial_vert,
    trivial_frag,
    static_vert,
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct TestResources {
    // N.B.: Field order is important
    pub(crate) empty_uniform_buffer: Arc<BufferAlloc>,
    pub(crate) empty_storage_buffer: Arc<BufferAlloc>,
    pub(crate) empty_image_2d: Arc<ImageView>,
    pub(crate) empty_storage_image_2d: Arc<ImageView>,
    pub(crate) empty_sampler: Arc<Sampler>,
    pub(crate) buffer_heap: Arc<BufferHeap>,
    pub(crate) image_heap: ImageHeap,
    pub(crate) samplers: SamplerCache,
    pub(crate) descriptors: Arc<DescriptorHeap>,
    pub(crate) shaders: TestShaders,
}

impl TestResources {
    pub(crate) fn new(device: &Arc<Device>) -> Self {
        let buffer_heap = BufferHeap::new(Arc::clone(device));
        let image_heap = ImageHeap::new(Arc::clone(device));
        let samplers = SamplerCache::new(Arc::clone(device));
        let descriptors = Arc::new(DescriptorHeap::new(device));
        let shaders = TestShaders::new(device);

        let empty_uniform_buffer = Arc::new(buffer_heap.alloc(
            BufferBinding::Uniform,
            Lifetime::Static,
            MemoryMapping::DeviceLocal,
            256,
        ));
        let empty_storage_buffer = Arc::new(buffer_heap.alloc(
            BufferBinding::Storage,
            Lifetime::Static,
            MemoryMapping::DeviceLocal,
            256,
        ));

        let empty_image_2d = ImageDef::new(
            &device,
            Default::default(),
            ImageType::Dim2,
            Format::RGBA8,
            SampleCount::One,
            (1, 1).into(),
            1,
            1,
        )
        .with_name("empty_image_2d")
        .build_image(&image_heap)
        .create_full_view();
        let empty_storage_image_2d = ImageDef::new(
            &device,
            ImageFlags::STORAGE | ImageFlags::NO_SAMPLE,
            ImageType::Dim2,
            Format::RGBA8,
            SampleCount::One,
            (1, 1).into(),
            1,
            1,
        )
        .with_name("empty_storage_image_2d")
        .build_image(&image_heap)
        .create_full_view();

        let desc = SamplerDesc {
            mag_filter: Filter::Linear,
            min_filter: Filter::Linear,
            mipmap_mode: SamplerMipmapMode::Linear,
            anisotropy_level: AnisotropyLevel::Sixteen,
            ..Default::default()
        };
        let empty_sampler = Arc::clone(&samplers.get_or_create(&desc));

        Self {
            buffer_heap,
            image_heap,
            samplers,
            descriptors,
            shaders,
            empty_uniform_buffer,
            empty_storage_buffer,
            empty_image_2d,
            empty_storage_image_2d,
            empty_sampler,
        }
    }

    pub(crate) fn device(&self) -> &Arc<Device> {
        self.image_heap.device()
    }
}

/// Render pass with a single subpass and single backbuffer attachment.
#[derive(Debug)]
pub(crate) struct TrivialPass {
    pub(crate) pass: Arc<RenderPass>,
    pub(crate) subpass: Subpass,
}

impl TrivialPass {
    pub(crate) fn new(device: &Arc<Device>) -> Self {
        unsafe { create_trivial_pass(Arc::clone(device)) }
    }

    pub(crate) fn create_framebuffers(&self, swapchain: &Swapchain) -> Vec<Arc<Framebuffer>> {
        unsafe {
            swapchain
                .create_views()
                .into_iter()
                .map(|view| Arc::new(Framebuffer::new(Arc::clone(&self.pass), vec![view.into()])))
                .collect()
        }
    }
}

unsafe fn create_trivial_pass(device: Arc<Device>) -> TrivialPass {
    use vk::ImageLayout as Layout;
    let pass = RenderPass::new(
        device,
        vec![AttachmentDescription {
            name: Attachment::Backbuffer,
            format: Format::BGRA8_SRGB,
            final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
            ..Default::default()
        }],
        vec![SubpassDesc {
            layouts: vec![Layout::COLOR_ATTACHMENT_OPTIMAL],
            color_attchs: vec![0],
            ..Default::default()
        }],
        vec![],
    );

    let mut subpasses = pass.subpasses();
    TrivialPass {
        pass: Arc::clone(&pass),
        subpass: subpasses.next().unwrap(),
    }
}

#[derive(Debug)]
pub(crate) struct TrivialRenderer {
    vert_shader: Arc<ShaderSpec>,
    frag_shader: Arc<ShaderSpec>,
    set_layouts: [Arc<SetLayout>; 2],
    descs: [DescriptorSet; 2],
}

const VERTEX_COUNT: u32 = 3;

impl TrivialRenderer {
    pub(crate) const fn vertex_count() -> u32 {
        VERTEX_COUNT
    }

    pub(crate) fn new(resources: &TestResources) -> Self {
        let device = resources.device();
        let dev = || Arc::clone(device);
        let descriptors = &resources.descriptors;
        let shaders = &resources.shaders;

        let layout0 = Arc::new(SetLayout::new(
            dev(),
            set_layout_desc![(0, UniformBuffer), (1, StorageBuffer),],
        ));
        let layout1 = Arc::new(SetLayout::new(
            dev(),
            set_layout_desc![
                (0, CombinedImageSampler),
                (1, StorageImage),
                (2, SampledImage),
            ],
        ));

        let vert_shader = Arc::new(Arc::clone(&shaders.trivial_vert).into());
        let frag_shader = Arc::new(Arc::clone(&shaders.trivial_frag).into());

        let mut desc0 = descriptors.alloc(Lifetime::Static, &layout0);
        desc0.write_buffer(0, resources.empty_uniform_buffer.range());
        desc0.write_buffer(1, resources.empty_storage_buffer.range());

        let mut desc1 = descriptors.alloc(Lifetime::Static, &layout1);
        unsafe {
            desc1.write_image(
                0,
                &resources.empty_image_2d,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                Some(&resources.empty_sampler),
            );
            desc1.write_image(
                1,
                &resources.empty_storage_image_2d,
                vk::ImageLayout::GENERAL,
                None,
            );
            desc1.write_image(
                2,
                &resources.empty_image_2d,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                None,
            );
        }
        let descs = [desc0, desc1];

        TrivialRenderer {
            set_layouts: [layout0, layout1],
            vert_shader,
            frag_shader,
            descs,
        }
    }

    pub(crate) fn init_pipe_desc(&self, desc: &mut GraphicsPipelineDesc) {
        desc.layout.set_layouts = self.set_layouts[..].into();
        desc.stages
            .insert(ShaderStage::Vertex, Arc::clone(&self.vert_shader));
        desc.stages
            .insert(ShaderStage::Fragment, Arc::clone(&self.frag_shader));
    }

    pub(crate) fn render(&self, pipelines: &PipelineCache, cmds: &mut SubpassCmds) {
        let mut desc = GraphicsPipelineDesc::new(cmds.subpass().clone());
        self.init_pipe_desc(&mut desc);

        let pipe = unsafe { pipelines.get_or_create_gfx(&desc) };
        cmds.bind_gfx_pipe(&pipe);

        cmds.bind_gfx_descs(0, &self.descs[0]);
        cmds.bind_gfx_descs(1, &self.descs[1]);

        unsafe {
            cmds.draw(Self::vertex_count(), 1);
        }
    }
}
