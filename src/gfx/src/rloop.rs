use std::sync::Arc;

use log::{debug, trace};
use prelude::*;

use crate::*;

#[derive(Debug)]
pub struct RenderLoop {
    device: Arc<Device>,
    gfx_queue: Arc<Queue>,
    globals: Arc<Globals>,
    frame_num: u64,
    swapchain: SwapchainControl,
    renderer: WorldRenderer,
    resources: ResourceSystem,
    master_sem: TimelineSemaphore,
    // This is declared last so that it will be dropped last
    state: Option<Box<SystemState>>,
}

#[derive(Debug)]
crate struct SwapchainControl {
    swapchain: Swapchain,
    acquired_image: Option<u32>,
    acquire_sem: BinarySemaphore,
    present_sem: BinarySemaphore,
}

impl Drop for RenderLoop {
    fn drop(&mut self) {
        self.wait_for_render();
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

        let mut resources = ResourceSystem::new(&gfx_queue);
        globals.upload_images(&mut resources);

        let renderer = WorldRenderer::new(
            &state,
            Arc::clone(&globals),
            &swapchain,
            Arc::clone(&gfx_queue),
        );

        let frame_num = 1;
        let swapchain = SwapchainControl::new(swapchain);

        let mut master_sem = TimelineSemaphore::new(
            Arc::clone(&device), frame_num);
        set_name!(master_sem);

        Ok(Self {
            device,
            gfx_queue,
            frame_num,
            swapchain,
            globals,
            renderer,
            resources,
            master_sem,
            state: Some(state),
        })
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.device
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

    pub fn define_image(
        &self,
        flags: ImageFlags,
        ty: ImageType,
        format: Format,
        extent: Extent3D,
        mip_levels: u32,
        layers: u32,
        name: Option<impl Into<String>>,
    ) -> Arc<ImageDef> {
        let mut def = ImageDef::new(
            self.device(),
            flags,
            ty,
            format,
            SampleCount::One,
            extent,
            mip_levels,
            layers,
        );
        if let Some(name) = name {
            def.set_name(name.into());
        }
        Arc::new(def)
    }

    pub fn get_image_state(&self, image: &Arc<ImageDef>) -> ResourceState {
        self.resources.get_image_state(image)
    }

    pub fn upload_image(
        &mut self,
        image: &Arc<ImageDef>,
        src: Arc<Vec<u8>>,
        src_offset: usize,
    ) {
        self.resources.upload_image(image, src, src_offset)
    }

    pub fn define_material(
        &self,
        program: MaterialProgram,
        images: MaterialImageBindings,
    ) -> Arc<MaterialDef> {
        self.renderer.materials().define_material(program, images)
    }

    pub fn get_or_create_sampler(&mut self, desc: &SamplerDesc) -> Arc<Sampler>
    {
        self.state().samplers.get_or_create(desc).into_owned()
    }

    crate fn new_frame(&mut self) {
        self.frame_num += 1;
        debug!("beginning frame {}", self.frame_num);
        self.state_mut().frame_over();
        let heap = &self.state.as_ref().unwrap().heap;
        self.resources.schedule(heap);
    }

    crate fn wait_for_render(&self) {
        let _ = self.master_sem.wait(self.frame_num, u64::MAX);
    }

    crate fn render(&mut self, world: RenderWorldData) {
        self.wait_for_render();
        self.new_frame();

        unsafe { self.swapchain.acquire(); }
        let state = Arc::new(self.state.take().unwrap());
        self.renderer.run(
            Arc::clone(&state),
            &self.resources,
            world,
            self.frame_num(),
            self.swapchain.image_index(),
            &mut self.swapchain.acquire_sem,
            &mut self.swapchain.present_sem,
            &mut self.master_sem,
        );
        self.state = Some(Arc::try_unwrap(state).unwrap());

        unsafe { self.swapchain.present(&self.gfx_queue); }
    }
}

impl SwapchainControl {
    fn new(swapchain: Swapchain) -> Self {
        let device = || Arc::clone(swapchain.device());
        let mut acquire_sem = BinarySemaphore::new(device());
        let mut present_sem = BinarySemaphore::new(device());
        set_name!(acquire_sem, present_sem);
        Self {
            swapchain,
            acquired_image: None,
            acquire_sem,
            present_sem,
        }
    }

    fn image_index(&self) -> u32 {
        self.acquired_image.unwrap()
    }

    fn swapchain_mut(&mut self) -> &mut Swapchain {
        &mut self.swapchain
    }

    unsafe fn acquire(&mut self) {
        trace!("SwapchainControl::acquire()");
        assert!(self.acquired_image.is_none());
        self.acquired_image = self.swapchain
            .acquire_next_image(&mut self.acquire_sem)
            .unwrap().into();
    }

    unsafe fn present(&mut self, present_queue: &Arc<Queue>) {
        trace!(
            "SwapchainControl::present(present_queue: {:?})",
            fmt_named(&**present_queue),
        );
        let index = self.image_index();
        present_queue.present(
            &[&mut self.present_sem],
            &mut self.swapchain,
            index,
        ).check().unwrap();
        self.acquired_image = None;
    }
}
