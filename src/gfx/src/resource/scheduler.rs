use std::sync::Arc;

use derive_more::From;
use log::{debug, trace};

use crate::device::{
    CmdBuffer, CmdBufferLevel, CmdPool, Device, ImageDef, ImageFlags,
    ImageHeap, ImageSubresources, Queue, SignalInfo, SubmitInfo,
    TimelineSemaphore, WaitResult, XferCmds,
};
use super::*;

#[derive(Debug)]
pub(super) struct UploadScheduler {
    queue: Arc<Queue>,
    tasks: TaskList,
    sem: TimelineSemaphore,
    avail_batch: u64,   // Memoized sem.get_value()
    pending_batch: u64,
    pool: Option<Box<CmdPool>>,
    cmds: vk::CommandBuffer,
}

/// Schedules the execution of GPU transfer commands.
#[derive(Debug)]
struct TaskList {
    staging: UploadStage,
    tasks: Vec<UploadTask>,
}

#[derive(Debug)]
crate struct ImageUploadTask {
    crate src: Arc<Vec<u8>>,
    crate src_offset: usize,
    crate image: Arc<ImageDef>,
    crate subresources: ImageSubresources,
}

#[derive(Debug, From)]
crate enum UploadTask {
    Image(ImageUploadTask),
}

impl TaskList {
    fn new(device: Arc<Device>) -> Self {
        let staging_buffer_size = 0x100_0000;
        let staging = UploadStage::new(device, staging_buffer_size);
        Self {
            staging,
            tasks: Vec::new(),
        }
    }

    fn push(&mut self, task: impl Into<UploadTask>) {
        self.tasks.push(task.into());
    }

    fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    fn process_tasks(
        &mut self,
        batch_num: u64,
        resources: &mut ResourceStateTable,
        heap: &ImageHeap,
        cmds: &mut XferCmds,
    ) {
        assert!(!self.tasks.is_empty());
        unsafe { self.staging.clear(); }

        // NB: Staging uploads is basically a bin-packing problem, but
        // we just use the most basic greedy algorithm possible.
        for i in (0..self.tasks.len()).rev() {
            let res = match &self.tasks[i] {
                UploadTask::Image(task) => upload_image(
                    batch_num, &mut self.staging, resources, heap, &task),
            };
            if res.is_ok() {
                self.tasks.remove(i);
            }
        }

        unsafe { self.staging.record_cmds(cmds); }
    }
}

// TODO: If a large subresource can't be uploaded at once, then
// upload just part of it.
fn upload_image(
    batch_num: u64,
    staging: &mut UploadStage,
    resources: &mut ResourceStateTable,
    heap: &ImageHeap,
    task: &ImageUploadTask,
) -> Result<(), StagingOutOfMemory> {
    assert!(!task.image.flags().contains(ImageFlags::NO_SAMPLE));
    let image = resources.prepare_for_upload(&task.image, batch_num, &heap);
    let buf = staging.stage_image(
        image,
        true,
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        vk::AccessFlags::SHADER_READ_BIT,
        task.subresources,
    )?;
    let start = task.src_offset;
    let end = start + buf.len();
    buf.copy_from_slice(&task.src[start..end]);
    Ok(())
}

impl UploadScheduler {
    crate fn new(queue: Arc<Queue>) -> Self {
        let mut sem = TimelineSemaphore::new(Arc::clone(queue.device()), 0);
        sem.set_name("upload_scheduler.sem");

        let mut pool = Box::new(CmdPool::new(
            queue.family(),
            vk::CommandPoolCreateFlags::TRANSIENT_BIT,
        ));
        pool.set_name("resource_system.pool");

        let cmds = pool.alloc(CmdBufferLevel::Primary);

        Self {
            tasks: TaskList::new(Arc::clone(queue.device())),
            queue,
            sem,
            avail_batch: 0,
            pending_batch: 0,
            pool: Some(pool),
            cmds,
        }
    }

    crate fn device(&self) -> &Arc<Device> {
        self.sem.device()
    }

    crate fn add_task(&mut self, task: impl Into<UploadTask>) {
        self.tasks.push(task);
    }

    crate fn avail_batch(&self) -> u64 {
        self.avail_batch
    }

    crate fn query_tasks(&mut self) -> SchedulerStatus {
        self.avail_batch = self.sem.get_value();
        let status = if self.avail_batch() < self.pending_batch {
            SchedulerStatus::Busy
        } else {
            SchedulerStatus::Idle
        };
        debug!("UploadScheduler::query_tasks: avail_batch = {}, status = {:?}",
            self.avail_batch, status);
        status
    }

    crate fn schedule(
        &mut self,
        resources: &mut ResourceStateTable,
        heap: &ImageHeap,
    ) {
        trace!("UploadScheduler::schedule()");

        assert_eq!(self.avail_batch(), self.pending_batch);
        if self.tasks.is_empty() { return; }

        self.pending_batch += 1;

        let mut pool = self.pool.take().unwrap();
        let mut cmds = unsafe {
            pool.reset();
            XferCmds::new(CmdBuffer::from_initial(
                pool, self.cmds, CmdBufferLevel::Primary))
        };
        self.tasks.process_tasks(
            self.pending_batch, resources, heap, &mut cmds);
        let (cmds, pool) = cmds.end();
        self.cmds = cmds;
        self.pool = Some(pool);

        let submissions = [SubmitInfo {
            cmds: &[self.cmds],
            sig_sems: &[SignalInfo {
                semaphore: self.sem.inner_mut(),
                value: self.pending_batch
            }],
            ..Default::default()
        }];
        unsafe { self.queue.submit(&submissions); }
    }

    crate fn wait_with_timeout(&mut self, timeout: u64) -> WaitResult {
        let res = self.sem.wait(self.pending_batch, timeout);
        if res == WaitResult::Success {
            self.avail_batch = self.pending_batch;
        }
        res
    }

    crate fn flush(
        &mut self,
        resources: &mut ResourceStateTable,
        heap: &ImageHeap,
    ) {
        trace!("ResourceScheduler::flush()");
        let _ = self.wait_with_timeout(u64::MAX);
        while !self.tasks.is_empty() {
            self.schedule(resources, heap);
            let _ = self.wait_with_timeout(u64::MAX);
        }
    }
}
