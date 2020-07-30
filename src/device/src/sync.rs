use std::convert::{TryFrom, TryInto};
use std::ptr;
use std::sync::Arc;

use derivative::Derivative;
use log::trace;

use crate::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
crate enum WaitResult {
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

#[derive(Derivative)]
#[derivative(Debug)]
crate struct SemaphoreInner {
    device: Arc<Device>,
    raw: vk::Semaphore,
    name: Option<String>,
}

#[derive(Debug)]
crate struct BinarySemaphore {
    inner: SemaphoreInner,
}

#[derive(Debug)]
crate struct TimelineSemaphore {
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
    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    crate fn raw(&self) -> vk::Semaphore {
        self.raw
    }

    fn set_name(&mut self, name: impl Into<String>) {
        let name: String = name.into();
        self.name = Some(name.clone());
        unsafe { self.device().set_name(self.raw, name); }
    }
}

impl Named for SemaphoreInner {
    fn name(&self) -> Option<&str> {
        Some(&self.name.as_ref()?)
    }
}

impl BinarySemaphore {
    crate fn new(device: Arc<Device>) -> Self {
        let dt = device.table();
        let create_info = vk::SemaphoreCreateInfo::default();
        let mut sem = vk::null();
        unsafe {
            dt.create_semaphore(&create_info, ptr::null(), &mut sem)
                .check().unwrap();
        }
        Self { inner: SemaphoreInner {
            device,
            raw: sem,
            name: None,
        } }
    }

    crate fn device(&self) -> &Arc<Device> {
        self.inner.device()
    }

    crate fn raw(&self) -> vk::Semaphore {
        self.inner.raw()
    }

    crate fn inner_mut(&mut self) -> &mut SemaphoreInner {
        &mut self.inner
    }

    crate fn set_name(&mut self, name: impl Into<String>) {
        self.inner.set_name(name);
    }
}

impl Named for BinarySemaphore {
    fn name(&self) -> Option<&str> {
        self.inner.name()
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
        let mut sem = vk::null();
        unsafe {
            dt.create_semaphore(&create_info, ptr::null(), &mut sem)
                .check().unwrap();
        }
        Self { inner: SemaphoreInner {
            device,
            raw: sem,
            name: None,
        } }
    }

    crate fn device(&self) -> &Arc<Device> {
        self.inner.device()
    }

    fn dt(&self) -> &vkl::DeviceTable {
        self.device().table()
    }

    crate fn raw(&self) -> vk::Semaphore {
        self.inner.raw()
    }

    crate fn inner_mut(&mut self) -> &mut SemaphoreInner {
        &mut self.inner
    }

    crate unsafe fn signal(&self, value: u64) {
        trace!("TimelineSemaphore::signal(self: {:?}, value: {})",
            fmt_named(self), value);
        self.dt().signal_semaphore(&vk::SemaphoreSignalInfo {
            semaphore: self.raw(),
            value,
            ..Default::default()
        });
    }

    crate fn wait(&self, value: u64, timeout: u64) -> WaitResult {
        trace!("TimelineSemaphore::wait(self: {:?}, value: {}, timeout: {})",
            fmt_named(self), value, timeout);
        unsafe {
            self.dt().wait_semaphores(&vk::SemaphoreWaitInfo {
                semaphore_count: 1,
                p_semaphores: &self.raw(),
                p_values: &value,
                ..Default::default()
            }, timeout).try_into().unwrap()
        }
    }

    crate fn get_value(&self) -> u64 {
        let mut value = 0;
        unsafe {
            self.dt().get_semaphore_counter_value(self.raw(), &mut value)
                .check().unwrap();
        }
        value
    }

    crate fn set_name(&mut self, name: impl Into<String>) {
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

        assert_eq!(sem.wait(9999, 1000), WaitResult::Timeout);
    }

    unsafe fn timeline_semaphore_queue_signal(vars: testing::TestVars) {
        let queue = vars.gfx_queue();
        let pool = Box::new(CmdPool::new_transient(queue.family()));

        let make_cmds = |pool|
            XferCmds::new(CmdBuffer::new_primary(pool)).end();
        let mut semaphore = TimelineSemaphore::new(
            Arc::clone(vars.device()), 0);

        // Test wait
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

        queue.device().wait_idle();
    }

    unit::declare_tests![
        timeline_semaphore_host_ops,
        timeline_semaphore_queue_signal,
    ];
}

unit::collect_tests![tests];
