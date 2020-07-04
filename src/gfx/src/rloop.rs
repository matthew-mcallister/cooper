use std::sync::Arc;

use log::debug;
use prelude::*;

use crate::*;

#[derive(Debug)]
crate struct SystemState {
    crate device: Arc<Device>,
    crate heap: DeviceHeap,
    crate buffers: Arc<BufferHeap>,
    crate descriptors: Arc<DescriptorHeap>,
    crate pipelines: PipelineCache,
    crate samplers: SamplerCache,
    //shader_specs: ..., (maybe)
}

#[derive(Debug)]
pub struct RenderLoop {
    device: Arc<Device>,
    gfx_queue: Arc<Queue>,
    swapchain: Swapchain,
    globals: Arc<Globals>,
    renderer: WorldRenderer,
    // TODO: All this stuff belongs under render/
    frame_num: u64,
    frame_in_flight: u64,
    swapchain_sem: BinarySemaphore,
    render_sem: BinarySemaphore,
    render_fence: Fence,
    // This is declared last so that it will be dropped last
    state: Option<Box<SystemState>>,
}

impl SystemState {
    crate fn new(device: Arc<Device>) -> Self {
        let dev = || Arc::clone(&device);
        let heap = DeviceHeap::new(dev());
        let buffers = BufferHeap::new(dev());
        let descriptors = Arc::new(DescriptorHeap::new(&device));
        let pipelines = PipelineCache::new(&device);
        let samplers = SamplerCache::new(dev());
        SystemState {
            device,
            heap,
            buffers,
            descriptors,
            pipelines,
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
            Arc::clone(&globals),
            &swapchain,
            scheduler,
        );

        let swapchain_sem = BinarySemaphore::new(Arc::clone(&device));
        let render_fence = Fence::new(Arc::clone(&device), false);
        let render_sem = BinarySemaphore::new(Arc::clone(&device));

        Ok(Self {
            device,
            gfx_queue,
            swapchain,
            globals,
            renderer,
            frame_num: 1,
            frame_in_flight: 0,
            swapchain_sem,
            render_sem,
            render_fence,
            state: Some(state),
        })
    }

    crate fn state(&self) -> &SystemState {
        &self.state.as_ref().unwrap()
    }

    fn state_mut(&mut self) -> &mut SystemState {
        &mut *self.state.as_mut().unwrap()
    }

    crate fn renderer(&self) -> &WorldRenderer {
        &self.renderer
    }

    crate fn frame_num(&self) -> u64 {
        self.frame_num
    }

    fn is_frame_in_flight(&self) -> bool {
        self.frame_in_flight == self.frame_num
    }

    pub fn create_material(
        &self,
        program: MaterialProgram,
        images: MaterialImageMap,
    ) -> Arc<Material> {
        self.renderer.materials().create_material(program, images)
    }

    fn finish_frame(&mut self) {
        // This method should prevent deadlocking in the destructor.
        if !self.is_frame_in_flight() { return; }
        debug!("waiting for frame {}", self.frame_num);
        self.render_fence.wait();
        self.render_fence.reset();
        self.frame_num += 1;
    }

    crate fn render(&mut self, world: RenderWorldData) {
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
                &[&mut self.render_sem],
                &mut self.swapchain,
                image_idx,
            ).check().unwrap();
        }
    }

    fn pre_render(&mut self) {
        let state = self.state_mut();
        unsafe {
            state.buffers.clear_frame();
            state.descriptors.clear_frame();
        }
        state.pipelines.commit();
        state.samplers.commit();
    }
}
