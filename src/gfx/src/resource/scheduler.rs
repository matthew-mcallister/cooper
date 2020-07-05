use std::sync::Arc;

use derive_more::From;

use crate::*;

#[derive(Debug)]
crate struct UploadScheduler {
    tasks: TaskProcessor,
    // TODO: Replace with timeline semaphore
    fence: Fence,
}

/// Schedules the execution of GPU transfer commands.
#[derive(Debug)]
struct TaskProcessor {
    staging: UploadStage,
    tasks: Vec<UploadTask>,
}

#[derive(Debug)]
crate struct ImageUploadTask {
    src: Arc<Vec<u8>>,
    src_offset: usize,
    image: Arc<Image>,
    subresources: ImageSubresources,
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

    fn process_tasks(&mut self, cmds: &mut XferCmds) {
        assert!(!self.tasks.is_empty());
        unsafe { self.staging.clear(); }

        // NB: Staging uploads is basically a bin-packing problem, but
        // we just use the most basic greedy algorithm possible.
        for i in (0..self.tasks.len()).rev() {
            let res = match &self.tasks[i] {
                UploadTask::Image(task) =>
                    upload_image(&mut self.staging, &task),
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
fn upload_image(staging: &mut UploadStage, task: &ImageUploadTask) ->
    Result<(), StagingOutOfMemory>
{
    assert!(!task.image.flags().contains(ImageFlags::NO_SAMPLE));
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
            fence: Fence::new(Arc::clone(&device), true),
            tasks: TaskProcessor::new(device),
        }
    }

    crate fn add_task(&mut self, task: impl Into<UploadTask>) {
        self.tasks.add_task(task);
    }

    crate fn schedule(&mut self, queue: &Queue, pool: Box<CmdPool>) ->
        Box<CmdPool>
    {
        if !self.fence.check_signaled() | self.tasks.is_empty() {
            return pool;
        }

        let mut cmds = XferCmds::new(CmdBuffer::new_primary(pool));
        self.tasks.process_tasks(&mut cmds);
        let (cmds, pool) = cmds.end();

        let submissions = [SubmitInfo {
            cmds: &[cmds],
            ..Default::default()
        }];
        self.fence.reset();
        unsafe { queue.submit(&submissions, Some(&mut self.fence)); }

        pool
    }

    crate fn wait_with_timeout(&self, timeout: u64) -> WaitResult {
        self.fence.wait_with_timeout(timeout)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use super::*;

    unsafe fn test_image(state: &SystemState, width: u32, height: u32) ->
        Arc<Image>
    {
        let extent = Extent3D::new(width, height, 1);
        Arc::new(Image::new(
            &state.heap,
            Default::default(),
            ImageType::Dim2,
            Format::RGBA8,
            SampleCount::One,
            extent,
            1,
            1,
        ))
    }

    unsafe fn upload(vars: testing::TestVars) {
        let device = vars.device();
        let mut uploads = UploadScheduler::new(Arc::clone(device));

        let state = SystemState::new(Arc::clone(&device));
        let images = [
            test_image(&state, 64, 64),
            test_image(&state, 128, 128),
        ];
        let mut pool = Box::new(CmdPool::new(
            vars.gfx_queue().family(),
            vk::CommandPoolCreateFlags::TRANSIENT_BIT,
        ));

        let mut data = Vec::new();
        data.resize(0x2_0000, 0u8);
        let data = Arc::new(data);

        for _ in 0..2 {
            for image in images.iter() {
                uploads.add_task(ImageUploadTask {
                    src: Arc::clone(&data),
                    src_offset: 0x1000,
                    image: Arc::clone(&image),
                    subresources: image.all_subresources(),
                });
            }
            pool = uploads.schedule(vars.gfx_queue(), pool);
            let _ = uploads.wait_with_timeout(2_000_000);
        }
    }

    unit::declare_tests![upload];
}

unit::collect_tests![tests];
