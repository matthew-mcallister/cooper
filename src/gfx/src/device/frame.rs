use std::sync::Arc;

use crate::*;

#[derive(Debug)]
crate struct FrameInfo {
    core: Arc<CoreData>,
    framebuffer: Arc<Framebuffer>,
    frame_num: u64,
}

impl FrameInfo {
    crate fn new(
        core: Arc<CoreData>,
        framebuffer: Arc<Framebuffer>,
        frame_num: u64,
    ) -> Self {
        FrameInfo {
            core,
            framebuffer,
            frame_num,
        }
    }

    crate fn core(&self) -> &Arc<CoreData> {
        &self.core
    }

    crate fn framebuffer(&self) -> &Arc<Framebuffer> {
        &self.framebuffer
    }

    crate fn frame_num(&self) -> u64 {
        self.frame_num
    }

    crate unsafe fn with_frame_locals<T>(
        &self,
        f: impl FnOnce(&mut FrameLocals<'_>) -> T,
    ) -> T {
        super::local::with_frame_locals(self, f)
    }
}
