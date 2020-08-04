use std::sync::Arc;

use device::{Device, Image, ImageDef, ImageHeap, Queue};

use super::*;

// TODO: This type is somewhat unnecessary
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

    #[allow(dead_code)]
    crate fn device(&self) -> &Arc<Device> {
        self.sched.device()
    }

    crate fn get_image_state(&self, image: &Arc<ImageDef>) -> ResourceState {
        self.state.get_state(image)
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
        self.state.get_image(image)
    }

    #[allow(dead_code)]
    crate fn invalidate_image(&mut self, image: &Arc<ImageDef>) {
        self.state.invalidate_image(image);
    }

    crate fn query_status(&mut self) -> SchedulerStatus {
        let status = self.sched.query_tasks();
        self.state.set_avail_batch(self.sched.avail_batch());
        status
    }

    crate fn schedule(&mut self, heap: &ImageHeap) {
        if self.query_status() == SchedulerStatus::Idle {
            self.sched.schedule(&mut self.state, heap);
        }
    }

    #[allow(dead_code)]
    crate fn flush(&mut self, heap: &ImageHeap) {
        self.sched.flush(&mut self.state, heap);
    }
}

#[cfg(test)]
mod tests {
    use device::*;
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

    unsafe fn upload(vars: crate::testing::TestVars) {
        let device = vars.device();
        let queue = vars.gfx_queue();

        let heap = &ImageHeap::new(Arc::clone(&device));
        let mut resources = ResourceSystem::new(queue);

        let images: Vec<_> = (0..7)
            .map(|n| test_image(device, 2 << n, 2 << n))
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

        macro_rules! check_available { () => {
            for image in images.iter() {
                assert_eq!(
                    resources.get_image_state(image),
                    ResourceState::Available,
                );
            }
        } };

        // Upload images one at a time
        for image in images.iter() {
            resources.upload_image(image, Arc::clone(&data), 0x1000);
            resources.flush(heap);
        }

        check_available!();

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

        check_available!();
    }

    unit::declare_tests![upload];
}

unit::collect_tests![tests];
