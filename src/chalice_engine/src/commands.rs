// TODO: Better support for long-lived command buffers.
// TODO: Wrapper around baked command buffers. Actually should be part
// of device crate.

use std::{cell::RefCell, sync::Arc};

use device::{CmdBuffer, CmdPool, Device};
use fnv::FnvHashMap;

use crate::Engine;

#[derive(Debug)]
struct AllocatedCommandBuffer {
    inner: vk::CommandBuffer,
    level: vk::CommandBufferLevel,
    last_used: u64,
}

/// Command pool for transient command buffers which caches and reuses
/// command buffers to automate management. For long-lived command
/// buffers, a regular command pool should be manually created and
/// allocated from.
// TODO probably: free old/unused command buffers
#[derive(Debug)]
struct CachedCommandPool {
    pool: CmdPool,
    cache_key: u64,
    free: Vec<AllocatedCommandBuffer>,
    allocated: Vec<AllocatedCommandBuffer>,
}

impl CachedCommandPool {
    fn new(device: &Arc<Device>, queue_family: u32) -> Self {
        let flags = vk::CommandPoolCreateFlags::TRANSIENT_BIT
            | vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER_BIT;
        let pool = CmdPool::new(device.queue_family(queue_family), flags);
        Self {
            pool,
            cache_key: 0,
            free: Vec::new(),
            allocated: Vec::new(),
        }
    }

    unsafe fn invalidate(&mut self, key: u64) {
        if self.cache_key < key {
            self.cache_key = key;
            self.free.extend(self.allocated.drain(..))
        }
        self.pool.reset();
    }

    fn try_alloc_free(&mut self, level: vk::CommandBufferLevel) -> Option<vk::CommandBuffer> {
        let idx = self.free.iter().position(|cmds| cmds.level == level)?;
        let mut cmds = self.free.remove(idx);
        cmds.last_used = self.cache_key;
        let inner = cmds.inner;
        self.allocated.push(cmds);
        Some(inner)
    }

    fn alloc_new(&mut self, level: vk::CommandBufferLevel) -> vk::CommandBuffer {
        let inner = self.pool.alloc(level);
        self.allocated.push(AllocatedCommandBuffer {
            inner,
            level,
            last_used: self.cache_key,
        });
        inner
    }

    fn alloc(&mut self, level: vk::CommandBufferLevel) -> vk::CommandBuffer {
        if let Some(inner) = self.try_alloc_free(level) {
            inner
        } else {
            self.alloc_new(level)
        }
    }

    fn pool_mut(&mut self) -> &mut CmdPool {
        &mut self.pool
    }
}

type Key = u32;

thread_local! {
    static POOLS: RefCell<FnvHashMap<Key, CachedCommandPool>> = Default::default();
}

pub(crate) fn with_command_buffer<R>(
    engine: &Engine,
    level: vk::CommandBufferLevel,
    queue_family: u32,
    f: impl FnOnce(device::CmdBuffer<'_>) -> R,
) -> R {
    POOLS.with(|pools| {
        let mut pools = pools.borrow_mut();
        if !pools.contains_key(&queue_family) {
            pools.insert(
                queue_family,
                CachedCommandPool::new(engine.device(), queue_family),
            );
        }
        let pool = pools.get_mut(&queue_family).unwrap();
        unsafe { pool.invalidate(engine.cache_key) };
        let inner = pool.alloc(level);
        let cmds = unsafe { CmdBuffer::from_initial(pool.pool_mut(), inner, level) };
        f(cmds)
    })
}
