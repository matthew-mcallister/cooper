use std::sync::Arc;

use parking_lot::Mutex;
use prelude::*;

use crate::*;

#[derive(Debug)]
crate struct SystemState {
    crate device: Arc<Device>,
    crate heap: DeviceHeap,
    crate buffers: Arc<BufferHeap>,
    crate descriptors: Mutex<DescriptorPool>, // TODO: Internal locking
    crate gfx_pipes: GraphicsPipelineCache,
    //compute_pipes: ...,
    crate samplers: SamplerCache,
    //shader_specs: ...,
}

impl SystemState {
    crate fn new(device: Arc<Device>) -> Self {
        let dev = || Arc::clone(&device);
        let heap = DeviceHeap::new(dev());
        let buffers = Arc::new(BufferHeap::new(dev()));
        let descriptors = Mutex::new(create_global_descriptor_pool(dev()));
        let gfx_pipes = GraphicsPipelineCache::new(dev());
        let samplers = SamplerCache::new(dev());
        SystemState {
            device,
            heap,
            buffers,
            descriptors,
            gfx_pipes,
            samplers,
        }
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }
}

#[derive(Debug)]
pub struct RenderLoop {
    device: Arc<Device>,
    gfx_queue: Arc<Queue>,
    swapchain: Swapchain,
    renderer: WorldRenderer,
    frame_num: u64,
    swapchain_sem: Semaphore,
    render_sem: Semaphore,
    render_fence: Fence,
    // This is declared last so that it will be dropped last
    state: Arc<SystemState>,
}

impl Drop for RenderLoop {
    fn drop(&mut self) {
        self.render_fence.wait();
        // For good measure
        self.device.wait_idle();
    }
}

impl RenderLoop {
    pub fn new(app_info: AppInfo, window: Arc<window::Window>) ->
        Result<Self, AnyError>
    {
        let (swapchain, queues) = unsafe { init_swapchain(app_info, window)? };
        let device = Arc::clone(&swapchain.device);
        let gfx_queue = Arc::clone(&queues[0][0]);

        let state = Arc::new(SystemState::new(Arc::clone(&device)));
        let globals = Arc::new(Globals::new(&state));

        let scheduler = Scheduler::new(Arc::clone(&gfx_queue));
        let renderer = WorldRenderer::new(
            &state,
            globals,
            &swapchain,
            scheduler,
        );

        let swapchain_sem = Semaphore::new(Arc::clone(&device));
        let render_fence = Fence::new(Arc::clone(&device), true);
        let render_sem = Semaphore::new(Arc::clone(&device));

        Ok(Self {
            device,
            gfx_queue,
            swapchain,
            renderer,
            frame_num: 0,
            swapchain_sem,
            render_sem,
            render_fence,
            state,
        })
    }

    pub fn do_frame(&mut self) {
        self.render_fence.wait();
        self.render_fence.reset();

        self.frame_num += 1;

        let image_idx = self.swapchain
            .acquire_next_image(&mut self.swapchain_sem)
            .unwrap();

        self.renderer.run(
            Arc::clone(&self.state),
            self.frame_num,
            image_idx,
            &mut self.swapchain_sem,
            &mut self.render_fence,
            &mut self.render_sem,
        );

        unsafe {
            self.gfx_queue.present(
                &[&self.render_sem],
                &mut self.swapchain,
                image_idx,
            ).check().unwrap();
        }

        self.update_state();
    }

    // Performs frame-end maintenance/bookkeeping.
    fn update_state(&mut self) {
        let state = Arc::get_mut(&mut self.state).unwrap();
        state.gfx_pipes.commit();
        state.samplers.commit();
    }
}
