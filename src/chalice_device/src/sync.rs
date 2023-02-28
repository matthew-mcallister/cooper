use std::convert::{TryFrom, TryInto};
use std::ptr;
use std::sync::Arc;

use derivative::Derivative;
use log::trace;

use crate::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub enum WaitResult {
    Success,
    Timeout,
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

impl WaitResult {
    pub fn unwrap(self) {
        if self != Self::Success {
            panic!("Semaphore wait timed out");
        }
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct SemaphoreInner {
    device: Arc<Device>,
    raw: vk::Semaphore,
    name: Option<String>,
}

#[derive(Debug)]
pub struct BinarySemaphore {
    inner: SemaphoreInner,
}

#[derive(Debug)]
pub struct TimelineSemaphore {
    inner: SemaphoreInner,
}

impl Drop for SemaphoreInner {
    fn drop(&mut self) {
        let dt = self.device.table();
        unsafe {
            dt.destroy_semaphore(self.raw, ptr::null());
        }
    }
}

impl SemaphoreInner {
    #[inline]
    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    #[inline]
    pub fn raw(&self) -> vk::Semaphore {
        self.raw
    }

    fn set_name(&mut self, name: impl Into<String>) {
        let name: String = name.into();
        self.name = Some(name.clone());
        unsafe {
            self.device().set_name(self.raw, name);
        }
    }
}

impl Named for SemaphoreInner {
    fn name(&self) -> Option<&str> {
        Some(&self.name.as_ref()?)
    }
}

impl BinarySemaphore {
    pub fn new(device: Arc<Device>) -> Self {
        let dt = device.table();
        let create_info = vk::SemaphoreCreateInfo::default();
        let mut sem = vk::null();
        unsafe {
            dt.create_semaphore(&create_info, ptr::null(), &mut sem)
                .check()
                .unwrap();
        }
        Self {
            inner: SemaphoreInner {
                device,
                raw: sem,
                name: None,
            },
        }
    }

    #[inline]
    pub fn device(&self) -> &Arc<Device> {
        self.inner.device()
    }

    #[inline]
    pub fn raw(&self) -> vk::Semaphore {
        self.inner.raw()
    }

    #[inline]
    pub fn inner(&self) -> &SemaphoreInner {
        &self.inner
    }

    #[inline]
    pub fn inner_mut(&mut self) -> &mut SemaphoreInner {
        &mut self.inner
    }

    #[inline]
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.inner.set_name(name);
    }
}

impl Named for BinarySemaphore {
    fn name(&self) -> Option<&str> {
        self.inner.name()
    }
}

impl TimelineSemaphore {
    pub fn new(device: Arc<Device>, value: u64) -> Self {
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
        let mut sem = vk::null();
        unsafe {
            dt.create_semaphore(&create_info, ptr::null(), &mut sem)
                .check()
                .unwrap();
        }
        Self {
            inner: SemaphoreInner {
                device,
                raw: sem,
                name: None,
            },
        }
    }

    #[inline]
    pub fn device(&self) -> &Arc<Device> {
        self.inner.device()
    }

    fn dt(&self) -> &vkl::DeviceTable {
        self.device().table()
    }

    #[inline]
    pub fn raw(&self) -> vk::Semaphore {
        self.inner.raw()
    }

    #[inline]
    pub fn inner_mut(&mut self) -> &mut SemaphoreInner {
        &mut self.inner
    }

    pub unsafe fn signal(&self, value: u64) {
        trace!(
            "TimelineSemaphore::signal(self: {:?}, value: {})",
            fmt_named(self),
            value
        );
        self.dt().signal_semaphore(&vk::SemaphoreSignalInfo {
            semaphore: self.raw(),
            value,
            ..Default::default()
        });
    }

    pub fn wait(&self, value: u64, timeout: u64) -> WaitResult {
        trace!(
            "TimelineSemaphore::wait(self: {:?}, value: {}, timeout: {})",
            fmt_named(self),
            value,
            timeout
        );
        unsafe {
            self.dt()
                .wait_semaphores(
                    &vk::SemaphoreWaitInfo {
                        semaphore_count: 1,
                        p_semaphores: &self.raw(),
                        p_values: &value,
                        ..Default::default()
                    },
                    timeout,
                )
                .try_into()
                .unwrap()
        }
    }

    pub fn get_value(&self) -> u64 {
        let mut value = 0;
        unsafe {
            self.dt()
                .get_semaphore_counter_value(self.raw(), &mut value)
                .check()
                .unwrap();
        }
        value
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.inner.set_name(name);
    }
}

impl Named for TimelineSemaphore {
    fn name(&self) -> Option<&str> {
        self.inner.name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;
    use std::sync::Arc;

    #[test]
    fn timeline_semaphore_host_ops() {
        let vars = TestVars::new();
        let device = Arc::clone(vars.device());

        let sem = Arc::new(TimelineSemaphore::new(device, 42));
        assert_eq!(sem.get_value(), 42);

        unsafe {
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

        assert_eq!(sem.wait(9999, 1000), WaitResult::Timeout);
    }

    #[test]
    fn timeline_semaphore_queue_signal() {
        let vars = TestVars::new();
        let queue = vars.gfx_queue();
        let pool = Box::new(CmdPool::new_transient(queue.family()));

        let make_cmds = |pool| {
            let mut cmds = CmdBuffer::new(pool);
            cmds.begin();
            cmds.end()
        };
        let mut semaphore = TimelineSemaphore::new(Arc::clone(vars.device()), 0);

        // Test wait
        unsafe {
            let (cmds, pool) = make_cmds(pool);
            let value = 1;
            queue.submit(&[SubmitInfo {
                sig_sems: &[SignalInfo {
                    semaphore: semaphore.inner_mut(),
                    value,
                }],
                cmds: &[cmds],
                ..Default::default()
            }]);
            let _ = semaphore.wait(value, u64::MAX);

            // Test get
            let (cmds, _pool) = make_cmds(pool);
            let value = 2;
            queue.submit(&[SubmitInfo {
                sig_sems: &[SignalInfo {
                    semaphore: semaphore.inner_mut(),
                    value,
                }],
                cmds: &[cmds],
                ..Default::default()
            }]);
            while semaphore.get_value() != value {
                std::thread::sleep(std::time::Duration::from_micros(100));
            }
        }

        queue.device().wait_idle();
    }
}
