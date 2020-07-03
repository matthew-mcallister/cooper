use std::convert::{TryFrom, TryInto};
use std::ptr;
use std::sync::Arc;

use crate::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
crate enum WaitResult {
    Success,
    Timeout,
}

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

    fn dt(&self) -> &vkl::DeviceTable {
        &self.device.table
    }

    crate fn inner(&self) -> vk::Fence {
        self.inner
    }

    crate fn wait(&self) {
        self.wait_with_timeout(u64::max_value());
    }

    crate fn wait_with_timeout(&self, timeout: u64) -> WaitResult {
        unsafe {
            let fences = [self.inner];
            self.dt().wait_for_fences(
                fences.len() as _,
                fences.as_ptr(),
                bool32(false),
                timeout,
            ).try_into().unwrap()
        }
    }

    crate fn check_signaled(&self) -> bool {
        unsafe {
            let res = self.dt().get_fence_status(self.inner);
            if res == vk::Result::SUCCESS {
                true
            } else {
                assert_eq!(res, vk::Result::NOT_READY);
                false
            }
        }
    }

    // TODO: This function hangs randomly---driver bug?
    crate fn reset(&mut self) {
        unsafe {
            let fences = [self.inner];
            self.dt().reset_fences(fences.len() as _, fences.as_ptr());
        }
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

impl TryFrom<vk::Result> for WaitResult {
    type Error = vk::Result;
    fn try_from(res: vk::Result) -> Result<Self, Self::Error> {
        match res {
            vk::Result::SUCCESS => Ok(Self::Success),
            vk::Result::TIMEOUT => Ok(Self::Timeout),
            _ => Err(res),
        }
    }
}
