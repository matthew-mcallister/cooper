use std::sync::Arc;

use derive_more::From;
use more_asserts::assert_lt;

use crate::{
    CmdBuffer, CmdPool, Device, Image, ImageFlags, ImageHeap,
    ImageSubresources, Queue, SubmitInfo, TimelineSemaphore, WaitResult,
    XferCmds,
};
use super::{ResourceStateTable, StagingOutOfMemory, UploadStage};

#[derive(Debug)]
pub(super) struct UploadScheduler {
    tasks: TaskProcessor,
    sem: TimelineSemaphore,
    avail_batch: u64,
    pending_batch: u64,
}

/// Schedules the execution of GPU transfer commands.
#[derive(Debug)]
struct TaskProcessor {
    staging: UploadStage,
    tasks: Vec<UploadTask>,
}

#[derive(Debug)]
crate struct ImageUploadTask {
    crate src: Arc<Vec<u8>>,
    crate src_offset: usize,
    crate image: Arc<Image>,
    crate subresources: ImageSubresources,
}

#[derive(Debug, From)]
crate enum UploadTask {
    Image(ImageUploadTask),
}

impl TaskProcessor {
    fn new(device: Arc<Device>) -> Self {
        let staging_buffer_size = 0x40_0000;
        let staging = UploadStage::new(device, staging_buffer_size);
        Self {
            staging,
            tasks: Vec::new(),
        }
    }

    fn add_task(&mut self, task: impl Into<UploadTask>) {
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
                    batch_num, &mut self.staging, resources, heap, &task,
                ),
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
    resources.prepare_for_upload(&task.image, batch_num, &heap);
    let buf = staging.stage_image(
        &task.image,
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
    crate fn new(device: Arc<Device>) -> Self {
        Self {
            tasks: TaskProcessor::new(Arc::clone(&device)),
            sem: TimelineSemaphore::new(device, 0),
            avail_batch: 0,
            pending_batch: 0,
        }
    }

    crate fn device(&self) -> &Arc<Device> {
        self.sem.device()
    }

    crate fn add_task(&mut self, task: impl Into<UploadTask>) {
        self.tasks.add_task(task);
    }

    crate fn avail_batch(&self) -> u64 {
        self.avail_batch
    }

    crate fn new_frame(&mut self) {
        self.avail_batch = self.sem.get_value();
    }

    crate fn schedule(
        &mut self,
        frame_num: u64,
        queue: &Queue,
        resources: &mut ResourceStateTable,
        heap: &ImageHeap,
        pool: Box<CmdPool>,
    ) -> Box<CmdPool> {
        assert_lt!(self.pending_batch, frame_num);
        if (self.avail_batch() < self.pending_batch) | self.tasks.is_empty() {
            return pool;
        }

        // TODO: Maybe this should should just inc by 1 each time?
        self.pending_batch = frame_num;

        let mut cmds = XferCmds::new(CmdBuffer::new_primary(pool));
        self.tasks.process_tasks(
            self.pending_batch,
            resources,
            heap,
            &mut cmds,
        );
        let (cmds, pool) = cmds.end();

        let submissions = [SubmitInfo {
            cmds: &[cmds],
            sig_sems: &[self.sem.inner()],
            sig_values: &[self.pending_batch],
            ..Default::default()
        }];
        unsafe { queue.submit(&submissions, None); }

        pool
    }

    crate fn wait_with_timeout(&self, timeout: u64) -> WaitResult {
        self.sem.wait(self.pending_batch, timeout)
    }
}
