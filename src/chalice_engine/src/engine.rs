use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use device::DeviceResult;
use log::{debug, info};

use crate::commands::with_command_buffer;
use crate::*;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Engine {
    queues: Vec<Vec<Arc<device::Queue>>>,
    swapchain: device::Swapchain,
    acquire_semaphore: device::BinarySemaphore,
    image_heap: device::ImageHeap,
    framebuffers: FramebufferCache,
    shaders: HashMap<String, Arc<device::Shader>>,
    pipelines: device::PipelineCache,
    pub(crate) cache_key: u64,
    // TODO: Staging buffer and (optional) upload queue
    // TODO?: image/(vertex) buffer caching system with garbage
    // collection. Possibly out of scope but solves a basic problem
    // while also ensuring images aren't deleted before frame is over.
}

impl Engine {
    pub fn from_window(
        app_info: device::AppInfo,
        window: &impl device::Window,
    ) -> DeviceResult<Self> {
        let (swapchain, queues) = device::init_device_and_swapchain(app_info, window)?;
        let device = swapchain.device();
        Ok(Self {
            queues,
            image_heap: device::ImageHeap::new(Arc::clone(device)),
            acquire_semaphore: device::BinarySemaphore::new(Arc::clone(device)),
            framebuffers: Default::default(),
            shaders: Default::default(),
            pipelines: device::PipelineCache::new(device),
            cache_key: 0,
            swapchain,
        })
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

    pub fn acquire_next_image(&mut self) -> DeviceResult<u32> {
        Ok(self
            .swapchain
            .acquire_next_image(&mut self.acquire_semaphore)?)
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
    }

    pub unsafe fn reclaim_transient_resources(&mut self) {
        self.cache_key += 1;
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
        with_command_buffer(self, level, queue_family, f)
    }
}
