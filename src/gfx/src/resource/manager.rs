use std::sync::Arc;

use crate::{CmdPool, Device, Image, ImageHeap, Queue, WaitResult};
use super::{ImageUploadTask, ResourceStateTable, UploadScheduler};

#[derive(Debug)]
crate struct ResourceManager {
    state: ResourceStateTable,
    sched: UploadScheduler,
}

impl ResourceManager {
    crate fn new(device: Arc<Device>) -> Self {
        Self {
            state: ResourceStateTable::new(),
            sched: UploadScheduler::new(device),
        }
    }

    crate fn device(&self) -> &Arc<Device> {
        self.sched.device()
    }

    crate fn register_image(&mut self, image: Arc<Image>) {
        self.state.register(image);
    }

    crate fn load_image(
        &mut self,
        image: &Arc<Image>,
        src: Arc<Vec<u8>>,
        src_offset: usize,
    ) {
        // Mipmap generation not available yet
        assert_eq!(image.mip_levels(), 1);
        self.sched.add_task(ImageUploadTask {
            src,
            src_offset,
            image: Arc::clone(image),
            subresources: image.all_subresources(),
        });
    }

    crate fn schedule(
        &mut self,
        frame_num: u64,
        queue: &Queue,
        heap: &ImageHeap,
        pool: Box<CmdPool>,
    ) -> Box<CmdPool> {
        self.sched.schedule(frame_num, queue, &mut self.state, heap, pool)
    }

    crate fn wait(&self, timeout: u64) -> WaitResult {
        self.sched.wait_with_timeout(timeout)
    }

    // TODO:
    // crate fn flush(...)
}

#[cfg(test)]
mod tests {
    use crate::*;
    use super::*;

    unsafe fn test_image(device: Arc<Device>, width: u32, height: u32) ->
        Arc<Image>
    {
        let extent = Extent3D::new(width, height, 1);
        Arc::new(Image::new(
            device,
            Default::default(),
            ImageType::Dim2,
            Format::RGBA8,
            SampleCount::One,
            extent,
            1,
            1,
        ))
    }

    unsafe fn upload(vars: testing::TestVars) {
        let device = vars.device();
        let queue = vars.gfx_queue();

        let state = SystemState::new(Arc::clone(&device));
        let images: Vec<_> = (0..7)
            .map(|n| test_image(Arc::clone(&device), 2 << n, 2 << n))
            .collect();
        let mut resources = ResourceManager::new(Arc::clone(device));

        let mut data = Vec::new();
        data.resize(0x2_0000, 0u8);
        let data = Arc::new(data);

        let mut pool = Box::new(CmdPool::new(
            vars.gfx_queue().family(),
            vk::CommandPoolCreateFlags::TRANSIENT_BIT,
        ));
        for frame in 1..images.len() + 1 {
            let image = &images[frame % images.len()];
            resources.register_image(Arc::clone(image));
            resources.load_image(image, Arc::clone(&data), 0x1000);
            pool = resources.schedule(frame as _, queue, &state.heap, pool);
            let _ = resources.wait(2_000_000);
        }
    }

    unit::declare_tests![upload];
}

unit::collect_tests![tests];
