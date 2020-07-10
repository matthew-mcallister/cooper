use std::sync::Arc;

use prelude::*;

use crate::*;

#[derive(Debug)]
pub struct RenderLoop {
    device: Arc<Device>,
    gfx_queue: Arc<Queue>,
    globals: Arc<Globals>,
    frame: FrameControl,
    renderer: WorldRenderer,
    resources: ResourceSystem,
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
        let resources = ResourceSystem::new(Arc::clone(&device));

        let frame = FrameControl::new(swapchain);

        Ok(Self {
            device,
            gfx_queue,
            globals,
            renderer,
            resources,
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

    pub fn create_image(
        &self,
        flags: ImageFlags,
        ty: ImageType,
        format: Format,
        extent: Extent3D,
        mip_levels: u32,
        layers: u32,
    ) -> Arc<Image> {
        unsafe {
            Arc::new(Image::new(
                Arc::clone(&self.device),
                flags,
                ty,
                format,
                SampleCount::One,
                extent,
                mip_levels,
                layers,
            ))
        }
    }

    pub fn get_image_state(&self, image: &Arc<Image>) -> ResourceState {
        self.resources.get_image_state(image)
    }

    pub fn upload_image(
        &mut self,
        image: &Arc<Image>,
        src: Arc<Vec<u8>>,
        src_offset: usize,
    ) {
        self.resources.upload_image(image, src, src_offset)
    }

    pub fn create_material(
        &self,
        program: MaterialProgram,
        images: MaterialImageBindings,
    ) -> Arc<Material> {
        self.renderer.materials()
            .create_material(self.state(), &self.globals, program, images)
    }

    crate fn new_frame(&mut self) {
        self.state_mut().frame_over();
        self.resources.new_frame();
    }

    crate fn render(&mut self, world: RenderWorldData) {
        self.frame.wait();
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
