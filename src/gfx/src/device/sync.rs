use std::convert::{TryFrom, TryInto};
use std::marker::PhantomData;
use std::ptr;
use std::sync::Arc;

use crate::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
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
crate struct BinarySemaphore {
    device: Arc<Device>,
    inner: vk::Semaphore,
}

#[derive(Debug)]
crate struct TimelineSemaphore {
    device: Arc<Device>,
    inner: vk::Semaphore,
}

// Basically a raw VkSemaphore but it borrows the owning object, thus
// ensuring that the user has unique access to the underlying semaphore.
#[derive(Debug)]
#[repr(transparent)]
crate struct SemaphoreInner<'sem> {
    raw: vk::Semaphore,
    _ph: PhantomData<&'sem mut BinarySemaphore>,
}

impl Drop for Fence {
    fn drop(&mut self) {
        let dt = self.device.table();
        unsafe {
            dt.destroy_fence(self.inner, ptr::null());
        }
    }
}

impl Fence {
    crate fn new(device: Arc<Device>, signaled: bool) -> Self {
        let dt = device.table();
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
        self.device.table()
    }

    crate fn inner(&self) -> vk::Fence {
        self.inner
    }

    crate fn wait(&self) {
        let _ = self.wait_with_timeout(u64::MAX);
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
    crate fn reset(&self) {
        unsafe {
            let fences = [self.inner];
            self.dt().reset_fences(fences.len() as _, fences.as_ptr());
        }
    }
}

impl Drop for BinarySemaphore {
    fn drop(&mut self) {
        let dt = self.device.table();
        unsafe {
            dt.destroy_semaphore(self.inner, ptr::null());
        }
    }
}

impl BinarySemaphore {
    crate fn new(device: Arc<Device>) -> Self {
        let dt = device.table();
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

    crate fn raw(&self) -> vk::Semaphore {
        self.inner
    }

    crate fn inner(&mut self) -> SemaphoreInner<'_> {
        SemaphoreInner { raw: self.inner, _ph: PhantomData }
    }
}

impl Drop for TimelineSemaphore {
    fn drop(&mut self) {
        let dt = self.device.table();
        unsafe {
            dt.destroy_semaphore(self.inner, ptr::null());
        }
    }
}

impl TimelineSemaphore {
    crate fn new(device: Arc<Device>, value: u64) -> Self {
        let dt = device.table();
        let ty_create_info = vk::SemaphoreTypeCreateInfo {
            semaphore_type: vk::SemaphoreType::TIMELINE,
            initial_value: value,
            ..Default::default()
        };
        let create_info = vk::SemaphoreCreateInfo {
            p_next: &ty_create_info as *const _ as _,
            ..Default::default()
        };
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

    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    fn dt(&self) -> &vkl::DeviceTable {
        self.device.table()
    }

    crate fn raw(&self) -> vk::Semaphore {
        self.inner
    }

    crate unsafe fn signal(&self, value: u64) {
        self.dt().signal_semaphore(&vk::SemaphoreSignalInfo {
            semaphore: self.inner,
            value,
            ..Default::default()
        });
    }

    crate fn wait(&self, value: u64, timeout: u64) -> WaitResult {
        unsafe {
            self.dt().wait_semaphores(&vk::SemaphoreWaitInfo {
                semaphore_count: 1,
                p_semaphores: &self.inner,
                p_values: &value,
                ..Default::default()
            }, timeout).try_into().unwrap()
        }
    }

    crate fn get_value(&self) -> u64 {
        let mut value = 0;
        unsafe {
            self.dt().get_semaphore_counter_value(self.inner, &mut value)
                .check().unwrap();
        }
        value
    }

    crate fn inner(&mut self) -> SemaphoreInner<'_> {
        SemaphoreInner { raw: self.inner, _ph: PhantomData }
    }
}

impl<'sem> From<SemaphoreInner<'sem>> for vk::Semaphore {
    fn from(inner: SemaphoreInner<'sem>) -> Self {
        inner.raw
    }
}

impl<'sem> SemaphoreInner<'sem> {
    crate fn slice_as_raw(slice: &[Self]) -> &[vk::Semaphore] {
        unsafe { std::mem::transmute(slice) }
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use crate::*;
    use super::*;

    unsafe fn timeline_semaphore_host_ops(vars: testing::TestVars) {
        let device = Arc::clone(vars.device());

        let sem = Arc::new(TimelineSemaphore::new(device, 42));
        assert_eq!(sem.get_value(), 42);

        sem.signal(43);
        assert_eq!(sem.get_value(), 43);
        sem.signal(45);
        assert_eq!(sem.get_value(), 45);
        assert_eq!(sem.wait(0, 1), WaitResult::Success);
        assert_eq!(sem.wait(45, 1), WaitResult::Success);

        let sem2 = Arc::clone(&sem);
        std::thread::spawn(move || {
            sem2.signal(80);
        });

        assert_eq!(sem.wait(80, 2_000_000), WaitResult::Success);
    }

    unit::declare_tests![
        timeline_semaphore_host_ops,
    ];
}

unit::collect_tests![tests];
