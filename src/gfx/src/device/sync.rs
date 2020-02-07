use std::ptr;
use std::sync::Arc;

use crate::*;

#[derive(Debug)]
crate struct Fence {
    device: Arc<Device>,
    inner: vk::Fence,
}

#[derive(Debug)]
crate struct Semaphore {
    device: Arc<Device>,
    inner: vk::Semaphore,
}

impl Drop for Fence {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            dt.destroy_fence(self.inner, ptr::null());
        }
    }
}

impl Fence {
    crate fn new(device: Arc<Device>, signaled: bool) -> Self {
        let dt = &*device.table;
        let mut create_info = vk::FenceCreateInfo::default();
        if signaled {
            create_info.flags |= vk::FenceCreateFlags::SIGNALED_BIT;
        }
        let mut inner = vk::null();
        unsafe {
            dt.create_fence(&create_info, ptr::null(), &mut inner)
                .check().unwrap();
        }
        Self {
            device,
            inner,
        }
    }

    crate fn inner(&self) -> vk::Fence {
        self.inner
    }

    crate fn wait(&self, timeout: u64) {
        let dt = &*self.device.table;
        unsafe {
            let fences = [self.inner];
            dt.wait_for_fences(
                fences.len() as _,
                fences.as_ptr(),
                bool32(false),
                timeout,
            );
        }
    }

    crate fn wait_forever(&self) {
        self.wait(u64::max_value());
    }
}

impl Drop for Semaphore {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            dt.destroy_semaphore(self.inner, ptr::null());
        }
    }
}

impl Semaphore {
    crate fn new(device: Arc<Device>) -> Self {
        let dt = &*device.table;
        let create_info = vk::SemaphoreCreateInfo::default();
        let mut inner = vk::null();
        unsafe {
            dt.create_semaphore(&create_info, ptr::null(), &mut inner)
                .check().unwrap();
        }
        Self {
            device,
            inner,
        }
    }

    crate fn inner(&self) -> vk::Semaphore {
        self.inner
    }
}
