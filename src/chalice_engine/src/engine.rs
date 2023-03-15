use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use device::DeviceResult;
use log::{debug, info};

use crate::*;

#[derive(Debug)]
pub struct Settings {
    staging_buffer_size: vk::DeviceSize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            staging_buffer_size: 8 * 1024 * 1024,
        }
    }
}

#[derive(Debug)]
pub struct Engine {
    settings: Settings,
    queues: Vec<Vec<Arc<device::Queue>>>,
    graphics_queue: Arc<device::Queue>,
    swapchain: device::Swapchain,
    swapchain_index: u32,
    acquire_semaphore: device::BinarySemaphore,
    buffer_heap: Arc<device::BufferHeap>,
    image_heap: device::ImageHeap,
    framebuffers: FramebufferCache,
    shaders: HashMap<String, Arc<device::Shader>>,
    pub(crate) cache_key: u64,
    pipelines: device::PipelineCache,
    set_layouts: device::DescriptorSetLayoutCache,
    descriptor_heap: Arc<device::DescriptorHeap>,
    samplers: device::SamplerCache,
    // TODO: Not a huge fan of mutexing this. A side effect of shoving
    // everything on one big struct.
    staging: Mutex<StagingBuffer>,
    // TODO: (optional) staging upload queue
    // TODO?: image/(vertex) buffer memory management with garbage
    // collection. Possibly out of scope but solves a basic problem
    // while also ensuring images aren't deleted before frame is over.
}

impl Engine {
    pub fn from_window(
        app_info: device::AppInfo,
        window: &impl device::Window,
        settings: Settings,
    ) -> DeviceResult<Self> {
        let (swapchain, queues) = device::init_device_and_swapchain(app_info, window)?;
        let device = swapchain.device();
        let graphics_queue = Arc::clone(&queues[0][0]);
        Ok(Self {
            queues,
            buffer_heap: device::BufferHeap::new(Arc::clone(device)),
            image_heap: device::ImageHeap::new(Arc::clone(device)),
            swapchain_index: 0,
            acquire_semaphore: device::BinarySemaphore::new(Arc::clone(device)),
            framebuffers: Default::default(),
            shaders: Default::default(),
            cache_key: 0,
            pipelines: device::PipelineCache::new(device),
            set_layouts: device::DescriptorSetLayoutCache::new(Arc::clone(device)),
            descriptor_heap: Arc::new(device::DescriptorHeap::new(device)),
            samplers: device::SamplerCache::new(Arc::clone(device)),
            staging: Mutex::new(StagingBuffer::new(
                Arc::clone(&graphics_queue),
                Arc::clone(&graphics_queue),
                settings.staging_buffer_size,
            )),
            graphics_queue,
            swapchain,
            settings,
        })
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn queues(&self) -> &[Vec<Arc<device::Queue>>] {
        &self.queues
    }

    pub fn swapchain(&self) -> &device::Swapchain {
        &self.swapchain
    }

    pub fn swapchain_mut(&mut self) -> &mut device::Swapchain {
        &mut self.swapchain
    }

    pub fn swapchain_index(&self) -> u32 {
        self.swapchain_index
    }

    pub fn swapchain_image(&self) -> &Arc<device::SwapchainView> {
        &self.swapchain().views()[self.swapchain_index as usize]
    }

    pub fn acquire_next_image(&mut self) -> DeviceResult<u32> {
        let index = self
            .swapchain
            .acquire_next_image(&mut self.acquire_semaphore)?;
        self.swapchain_index = index;
        Ok(index)
    }

    pub fn present(&mut self, wait_semaphores: &[&mut device::BinarySemaphore]) {
        unsafe {
            self.graphics_queue
                .present(wait_semaphores, &mut self.swapchain, self.swapchain_index);
        }
    }

    pub fn acquire_semaphore_mut(&mut self) -> &mut device::BinarySemaphore {
        &mut self.acquire_semaphore
    }

    pub fn device(&self) -> &Arc<device::Device> {
        self.swapchain.device()
    }

    pub fn device_ref(&self) -> Arc<device::Device> {
        Arc::clone(self.swapchain.device())
    }

    pub fn image_heap(&self) -> &device::ImageHeap {
        &self.image_heap
    }

    pub fn pipelines(&self) -> &device::PipelineCache {
        &self.pipelines
    }

    pub fn pipelines_mut(&mut self) -> &mut device::PipelineCache {
        &mut self.pipelines
    }

    /// Does top-of-frame housekeeping.
    pub fn new_frame(&mut self) {
        self.pipelines.commit();
        self.set_layouts.commit();
        self.samplers.commit();
    }

    pub unsafe fn reclaim_transient_resources(&mut self) {
        self.cache_key += 1;
        self.buffer_heap.clear_frame();
        self.descriptor_heap.clear_frame();
    }

    /// Begins a render pass but creates the framebuffer for you lazily.
    pub fn begin_render_pass(
        &self,
        cmds: &mut device::CmdBuffer,
        render_pass: &Arc<device::RenderPass>,
        attachments: &[device::AttachmentImage],
        clear_values: &[vk::ClearValue],
    ) {
        let fb = self.framebuffers.get_or_create(render_pass, attachments);
        cmds.begin_render_pass(fb, clear_values, device::SubpassContents::Inline);
    }

    pub fn load_shaders_from_dir(&mut self, dir: impl AsRef<Path>) -> io::Result<()> {
        debug!("Loading shaders from {:?}", dir.as_ref());

        fn path_to_string(path: PathBuf) -> io::Result<String> {
            path.into_os_string()
                .into_string()
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "Invalid filename"))
        }

        let dir = dir.as_ref();
        let glob = dir.to_str().unwrap().to_owned() + "/**/*.spv";
        for entry in glob::glob(&glob).unwrap() {
            let path = entry.map_err(|e| e.into_error())?;
            if !path.is_file() {
                continue;
            }

            info!("Loading shader {:?}", path);

            let key = path_to_string(path.strip_prefix(dir).unwrap().with_extension(""))?;
            let path_str = path_to_string(path)?;
            let shader = unsafe {
                Arc::new(device::Shader::from_path(
                    Arc::clone(self.device()),
                    path_str,
                )?)
            };

            self.shaders.insert(key, shader);
        }
        Ok(())
    }

    pub fn get_shader(&self, name: &str) -> Option<&Arc<device::Shader>> {
        self.shaders.get(name)
    }

    pub fn with_command_buffer<R>(
        &self,
        level: vk::CommandBufferLevel,
        queue_family: u32,
        f: impl FnOnce(device::CmdBuffer<'_>) -> R,
    ) -> R {
        commands::with_command_buffer(self, level, queue_family, f)
    }

    pub fn buffer_heap(&self) -> &Arc<device::BufferHeap> {
        &self.buffer_heap
    }

    pub fn descriptor_set_layouts(&self) -> &device::DescriptorSetLayoutCache {
        &self.set_layouts
    }

    pub fn descriptor_heap(&self) -> &Arc<device::DescriptorHeap> {
        &self.descriptor_heap
    }

    pub fn create_descriptor_set<'r>(
        &self,
        lifetime: device::Lifetime,
        name: Option<impl Into<String>>,
        resources: &[DescriptorResource<'r>],
    ) -> device::DescriptorSet {
        create_descriptor_set(
            &self.set_layouts,
            &self.descriptor_heap,
            lifetime,
            name.map(Into::into),
            resources,
        )
    }

    pub fn samplers(&self) -> &device::SamplerCache {
        &self.samplers
    }

    pub fn samplers_mut(&mut self) -> &device::SamplerCache {
        &mut self.samplers
    }

    pub fn staging(&self) -> &Mutex<StagingBuffer> {
        &self.staging
    }

    pub fn staging_mut(&mut self) -> &mut StagingBuffer {
        self.staging.get_mut().unwrap()
    }
}
