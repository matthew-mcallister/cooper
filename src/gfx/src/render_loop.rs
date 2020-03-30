use std::sync::Arc;

use log::debug;
use prelude::*;

use crate::*;

// TODO: Some things may not benefit from multithreading in the long run
#[derive(Debug)]
crate struct SystemState {
    crate device: Arc<Device>,
    crate heap: DeviceHeap,
    crate buffers: Arc<BufferHeap>,
    crate descriptors: Arc<DescriptorHeap>,
    crate gfx_pipes: GraphicsPipelineCache,
    //compute_pipes: ...,
    crate samplers: SamplerCache,
    //shader_specs: ..., (maybe)
}

#[derive(Debug)]
pub struct RenderLoop {
    device: Arc<Device>,
    gfx_queue: Arc<Queue>,
    swapchain: Swapchain,
    renderer: WorldRenderer,
    frame_num: u64,
    frame_in_flight: u64,
    swapchain_sem: Semaphore,
    render_sem: Semaphore,
    render_fence: Fence,
    // This is declared last so that it will be dropped last
    crate state: Option<Box<SystemState>>,
}

impl SystemState {
    crate fn new(device: Arc<Device>) -> Self {
        let dev = || Arc::clone(&device);
        let heap = DeviceHeap::new(dev());
        let buffers = BufferHeap::new(dev());
        let descriptors = Arc::new(DescriptorHeap::new(dev()));
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

impl Drop for RenderLoop {
    fn drop(&mut self) {
        self.finish_frame();
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

        let state = Box::new(SystemState::new(Arc::clone(&device)));
        let globals = Arc::new(Globals::new(&state));

        let scheduler = Scheduler::new(Arc::clone(&gfx_queue));
        let renderer = WorldRenderer::new(
            &state,
            globals,
            &swapchain,
            scheduler,
        );

        let swapchain_sem = Semaphore::new(Arc::clone(&device));
        let render_fence = Fence::new(Arc::clone(&device), false);
        let render_sem = Semaphore::new(Arc::clone(&device));

        Ok(Self {
            device,
            gfx_queue,
            swapchain,
            renderer,
            frame_num: 1,
            frame_in_flight: 0,
            swapchain_sem,
            render_sem,
            render_fence,
            state: Some(state),
        })
    }

    crate fn frame_num(&self) -> u64 {
        self.frame_num
    }

    fn is_frame_in_flight(&self) -> bool {
        self.frame_in_flight == self.frame_num
    }

    fn finish_frame(&mut self) {
        // This method should prevent deadlocking in the destructor.
        if !self.is_frame_in_flight() { return; }
        debug!("waiting for frame {}", self.frame_num);
        self.render_fence.wait();
        self.render_fence.reset();
        self.frame_num += 1;
    }

    pub fn render(&mut self, mut world: RenderWorld) {
        self.state = world.state.take();

        self.finish_frame();

        debug!("beginning frame {}", self.frame_num);
        self.pre_render();

        let image_idx = self.swapchain
            .acquire_next_image(&mut self.swapchain_sem)
            .unwrap();

        let state = Arc::new(self.state.take().unwrap());
        self.frame_in_flight += 1;
        self.renderer.run(
            Arc::clone(&state),
            world,
            self.frame_num,
            image_idx,
            &mut self.swapchain_sem,
            &mut self.render_fence,
            &mut self.render_sem,
        );
        self.state = Some(Arc::try_unwrap(state).unwrap());

        unsafe {
            self.gfx_queue.present(
                &[&self.render_sem],
                &mut self.swapchain,
                image_idx,
            ).check().unwrap();
        }
    }

    fn pre_render(&mut self) {
        let state = self.state.as_mut().unwrap();
        state.gfx_pipes.commit();
        state.samplers.commit();
    }
}
