use std::sync::Arc;

use log::debug;

use crate::*;

// Handles timing, synchronization, and presentation of each rendered
// frame.
#[derive(Debug)]
crate struct FrameControl {
    frame_num: u64,
    swapchain: Swapchain,
    image_idx: u32,
    crate acquire_sem: BinarySemaphore,
    crate present_sem: BinarySemaphore,
    crate master_sem: TimelineSemaphore,
}

impl Drop for FrameControl {
    fn drop(&mut self) {
        self.wait();
    }
}

impl FrameControl {
    crate fn new(swapchain: Swapchain) -> Self {
        let device = || Arc::clone(swapchain.device());
        let frame_num = 1;
        let acquire_sem = BinarySemaphore::new(device());
        let present_sem = BinarySemaphore::new(device());
        let master_sem = TimelineSemaphore::new(device(), frame_num);
        Self {
            frame_num: 1,
            swapchain,
            image_idx: 0,
            acquire_sem,
            present_sem,
            master_sem,
        }
    }

    crate fn frame_num(&self) -> u64 {
        self.frame_num
    }

    crate fn image_index(&self) -> u32 {
        self.image_idx
    }

    crate fn pending(&self) -> bool {
        self.master_sem.get_value() != self.frame_num
    }

    crate fn swapchain_mut(&mut self) -> &mut Swapchain {
        &mut self.swapchain
    }

    crate fn wait(&self) {
        let _ = self.master_sem.wait(self.frame_num, u64::MAX);
    }

    crate fn acquire(&mut self) {
        debug!("FrameControl::begin(self: {:?})", self);
        assert!(!self.pending());
        self.image_idx = self.swapchain
            .acquire_next_image(&mut self.acquire_sem)
            .unwrap();
        self.frame_num += 1;
    }

    crate fn present(&mut self, present_queue: &Arc<Queue>) {
        debug!(
            "FrameControl::finish(self: {:?}, present_queue: {:?})",
            self, present_queue,
        );
        unsafe {
            present_queue.present(
                &[&mut self.present_sem],
                &mut self.swapchain,
                self.image_idx,
            ).check().unwrap();
        }
    }
}
