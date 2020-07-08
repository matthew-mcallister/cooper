use std::sync::Arc;

use crate::{CmdPool, Device, Image, ImageHeap, Queue, WaitResult};
use super::{
    ImageUploadTask, ResourceState, ResourceStateTable, UploadScheduler,
};

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

    crate fn new_frame(&mut self) {
        self.sched.new_frame();
    }

    crate fn get_image_state(&self, image: &Arc<Image>) -> ResourceState {
        self.state.get_state(image, self.sched.avail_batch())
    }

    crate fn upload_image(
        &mut self,
        image: &Arc<Image>,
        src: Arc<Vec<u8>>,
        src_offset: usize,
    ) {
        // Mipmap generation not available yet
        assert_eq!(image.mip_levels(), 1);
        self.state.register(image);
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

        for image in images.iter() {
            assert_eq!(
                resources.get_image_state(image),
                ResourceState::Unavailable,
            );
        }

        let mut data = Vec::new();
        data.resize(0x2_0000, 0u8);
        let data = Arc::new(data);

        let mut pool = Box::new(CmdPool::new(
            vars.gfx_queue().family(),
            vk::CommandPoolCreateFlags::TRANSIENT_BIT,
        ));

        // Simulate uploading N images, one at a time, and waiting on
        // them in a loop.
        let mut frame = 1;
        for image in images.iter() {
            resources.upload_image(image, Arc::clone(&data), 0x1000);

            loop {
                frame += 1;
                resources.new_frame();

                pool = resources.schedule(
                    frame as _, queue, &state.heap, pool);

                let state = resources.get_image_state(image);
                if state == ResourceState::Available {
                    break;
                } else {
                    assert_eq!(state, ResourceState::Pending);
                }

                let _ = resources.wait(1_000_000);
            }
        }

        for image in images.iter() {
            assert_eq!(
                resources.get_image_state(image),
                ResourceState::Available,
            );
        }
    }

    unit::declare_tests![upload];
}

unit::collect_tests![tests];
