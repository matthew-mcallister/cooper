use std::cell::RefCell;
use std::ptr;
use std::sync::Arc;

use ccore::name::*;

use crate::*;

thread_local! {
    static THREAD_DATA: RefCell<Option<ThreadData>> = Default::default();
}

#[derive(Debug)]
crate struct ThreadData {
    frame_data: [FrameLocalData; 2],
}

impl ThreadData {
    crate fn new(core: &CoreData) -> Self {
        ThreadData {
            frame_data: [
                FrameLocalData::new(core),
                FrameLocalData::new(core),
            ],
        }
    }

    crate fn get_frame_mut(&mut self, frame_num: u64) -> &mut FrameLocalData {
        let idx = frame_num % self.frame_data.len() as u64;
        &mut self.frame_data[idx as usize]
    }
}

/// Thread-local, per-frame state, mainly for distributing resource
/// allocation (memory, command buffers) among threads.
#[derive(Debug)]
crate struct FrameLocalData {
    device: Arc<Device>,
    frame_num: u64,
    cmd_pool: vk::CommandPool,
    // TODO: Device allocator
    // TODO: Local allocators
}

impl Drop for FrameLocalData {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            // Must wait here since we don't know when thread locals are
            // going to be destroyed.
            // TODO maybe: destroy thread locals when render loop is
            // destroyed rather than letting the runtime do it lazily.
            dt.device_wait_idle();
            dt.destroy_command_pool(self.cmd_pool, ptr::null());
        }
    }
}

impl FrameLocalData {
    crate fn new(core: &CoreData) -> Self {
        let device = Arc::clone(core.device());
        let dt = &*device.table;

        let create_info = vk::CommandPoolCreateInfo {
            flags: vk::CommandPoolCreateFlags::TRANSIENT_BIT,
            queue_family_index: core.gfx_queue().family().index(),
            ..Default::default()
        };
        let mut cmd_pool = vk::null();
        unsafe {
            dt.create_command_pool(&create_info, ptr::null(), &mut cmd_pool);
        }

        FrameLocalData {
            device,
            frame_num: 0,
            cmd_pool,
        }
    }

    crate unsafe fn update_frame(&mut self, frame_num: u64) {
        if self.frame_num == frame_num { return; }
        self.frame_num = frame_num;
        let dt = &*self.device.table;
        dt.reset_command_pool(self.cmd_pool, Default::default());
    }
}

#[derive(Debug)]
crate struct FrameLocals<'a> {
    data: &'a mut FrameLocalData,
    frame: &'a FrameInfo,
}

pub(super) unsafe fn with_frame_locals<T>(
    frame: &FrameInfo,
    f: impl FnOnce(&mut FrameLocals<'_>) -> T,
) -> T {
    THREAD_DATA.with(|local| {
        let mut local_borrow = local.borrow_mut();
        let local = local_borrow
            .get_or_insert_with(|| ThreadData::new(frame.core()));
        let data = local.get_frame_mut(frame.frame_num());
        data.update_frame(frame.frame_num());
        let mut frame = FrameLocals { data, frame };
        f(&mut frame)
    })
}

impl<'a> FrameLocals<'a> {
    crate unsafe fn create_subpass_cmds(
        &mut self,
        pass: Name,
        subpass: Name,
    ) -> SubpassCmds {
        let create_info = SubpassCmdsCreateInfo {
            core: Arc::clone(&self.frame.core()),
            pool: self.data.cmd_pool,
            framebuffer: Arc::clone(&self.frame.framebuffer()),
            pass,
            subpass,
        };
        SubpassCmds::new(create_info)
    }
}
