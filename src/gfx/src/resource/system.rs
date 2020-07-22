use std::sync::Arc;

use crate::device::{Device, Image, ImageDef, ImageHeap, Queue};
use super::*;

#[derive(Debug)]
crate struct ResourceSystem {
    state: ResourceStateTable,
    sched: UploadScheduler,
}

impl ResourceSystem {
    crate fn new(queue: &Arc<Queue>) -> Self {
        Self {
            state: ResourceStateTable::new(),
            sched: UploadScheduler::new(Arc::clone(queue)),
        }
    }

    crate fn device(&self) -> &Arc<Device> {
        self.sched.device()
    }

    crate fn get_image_state(&self, image: &Arc<ImageDef>) -> ResourceState {
        self.state.get_state(image, self.sched.avail_batch())
    }

    // TODO: For stuff like storage images, it could potentially be
    // useful to be able to do ad-hoc layout transitions with no upload.
    crate fn upload_image(
        &mut self,
        image: &Arc<ImageDef>,
        src: Arc<Vec<u8>>,
        src_offset: usize,
    ) {
        // Mipmap generation not available yet
        assert_eq!(image.mip_levels(), 1);
        assert!(!image.flags().is_attachment());
        self.sched.add_task(ImageUploadTask {
            src,
            src_offset,
            image: Arc::clone(image),
            subresources: image.all_subresources(),
        });
    }

    crate fn get_image(&self, image: &Arc<ImageDef>) -> Option<&Arc<Image>> {
        self.state.get_image(image, self.sched.avail_batch())
    }

    crate fn invalidate_image(&mut self, image: &Arc<ImageDef>) {
        self.state.invalidate_image(image);
    }

    crate fn schedule(&mut self, heap: &ImageHeap) {
        if self.sched.query_tasks() == SchedulerStatus::Idle {
            self.sched.schedule(&mut self.state, heap);
        }
    }

    crate fn flush(&mut self, heap: &ImageHeap) {
        self.sched.flush(&mut self.state, heap);
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use super::*;

    unsafe fn test_image(device: &Arc<Device>, width: u32, height: u32) ->
        Arc<ImageDef>
    {
        let extent = Extent3D::new(width, height, 1);
        Arc::new(ImageDef::new(
            device,
            Default::default(),
            ImageType::Dim2,
            Format::RGBA8,
            SampleCount::One,
            extent,
            1,
            1,
        ).with_name(format!("{}x{}", width, height)))
    }

    unsafe fn upload(vars: testing::TestVars) {
        let device = vars.device();
        let queue = vars.gfx_queue();

        let state = SystemState::new(Arc::clone(&device));
        let heap = &state.heap;
        let mut resources = ResourceSystem::new(queue);

        let images: Vec<_> = (0..7)
            .map(|n| test_image(&device, 2 << n, 2 << n))
            .collect();
        for image in images.iter() {
            assert_eq!(
                resources.get_image_state(image),
                ResourceState::Unavailable,
            );
        }

        let mut data = Vec::new();
        data.resize(0x2_0000, 0u8);
        let data = Arc::new(data);

        // Upload images one at a time
        for image in images.iter() {
            resources.upload_image(image, Arc::clone(&data), 0x1000);
            resources.flush(heap);
        }

        for image in images.iter() {
            assert_eq!(
                resources.get_image_state(image),
                ResourceState::Available,
            );
        }

        // Upload several images at once
        for image in images.iter() {
            resources.invalidate_image(image);
            assert_eq!(
                resources.get_image_state(image),
                ResourceState::Unavailable,
            );

            resources.upload_image(image, Arc::clone(&data), 0x1000);
        }
        resources.flush(heap);
    }

    unit::declare_tests![upload];
}

unit::collect_tests![tests];
