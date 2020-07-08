use std::sync::Arc;

use prelude::*;

use crate::{
    AppInfo, Device, FrameControl, Globals, Material, MaterialImageMap,
    MaterialProgram, Queue, RenderWorldData, SystemState, WorldRenderer,
};
use crate::init_swapchain;

#[derive(Debug)]
pub struct RenderLoop {
    device: Arc<Device>,
    gfx_queue: Arc<Queue>,
    globals: Arc<Globals>,
    renderer: WorldRenderer,
    frame: FrameControl,
    // This is declared last so that it will be dropped last
    state: Option<Box<SystemState>>,
}

impl Drop for RenderLoop {
    fn drop(&mut self) {
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

        let renderer = WorldRenderer::new(
            &state,
            Arc::clone(&globals),
            &swapchain,
            Arc::clone(&gfx_queue),
        );

        let frame = FrameControl::new(swapchain);

        Ok(Self {
            device,
            gfx_queue,
            globals,
            renderer,
            frame,
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
        self.frame.frame_num()
    }

    pub fn create_material(
        &self,
        program: MaterialProgram,
        images: MaterialImageMap,
    ) -> Arc<Material> {
        self.renderer.materials().create_material(program, images)
    }

    crate fn render(&mut self, world: RenderWorldData) {
        self.frame.wait();
        self.state_mut().frame_over();
        self.frame.acquire();

        let state = Arc::new(self.state.take().unwrap());
        self.renderer.run(
            Arc::clone(&state),
            world,
            self.frame_num(),
            self.frame.image_index(),
            &mut self.frame.acquire_sem,
            &mut self.frame.present_sem,
            &mut self.frame.master_sem,
        );
        self.state = Some(Arc::try_unwrap(state).unwrap());

        self.frame.present(&self.gfx_queue);
    }
}
