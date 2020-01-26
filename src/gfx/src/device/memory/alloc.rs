use std::ops::Range;

use prelude::*;

use super::*;

/// Address-ordered FIFO allocation algorithm.
#[derive(Debug, Default)]
pub(super) struct FreeListAllocator {
    capacity: vk::DeviceSize,
    used: vk::DeviceSize,
    // List of chunk sizes
    chunks: Vec<vk::DeviceSize>,
    free: Vec<Block>,
}

impl FreeListAllocator {
    pub(super) fn new() -> Self {
        Default::default()
    }

    pub(super) fn used(&self) -> vk::DeviceSize {
        self.used
    }

    pub(super) fn capacity(&self) -> vk::DeviceSize {
        self.capacity
    }

    pub(super) fn add_chunk(&mut self, size: vk::DeviceSize) {
        self.capacity += size;
        self.chunks.push(size);
        self.free.push(Block {
            chunk: (self.chunks.len() - 1) as _,
            start: 0,
            end: size,
        });
    }

    pub(super) fn carve_block(
        &mut self,
        index: usize,
        range: Range<vk::DeviceSize>,
    ) {
        self.used += range.end - range.start;

        let old_block = self.free[index];
        debug_assert!(old_block.start <= range.start &&
            range.end <= old_block.end);
        debug_assert!(range.start < range.end);

        // Resize/cull old block
        let mut block = &mut self.free[index];
        block.start = range.end;
        // TODO: Reverse free list order to prefer removal near end
        if block.is_empty() { self.free.remove(index); }

        // Insert padding block if necessary
        let chunk_idx = old_block.chunk;
        if range.start > old_block.start {
            let block = Block {
                chunk: chunk_idx,
                start: old_block.start,
                end: range.start,
            };
            self.free.insert(index, block);
        }
    }

    pub(super) fn alloc_in(
        &mut self,
        block_idx: usize,
        size: vk::DeviceSize,
        alignment: vk::DeviceSize,
    ) -> Option<Block> {
        let block = &self.free[block_idx];
        let offset = align(alignment, block.start);
        if offset + size > block.end { return None; }
        let chunk = block.chunk;
        self.carve_block(block_idx, offset..offset + size);
        Some(Block {
            chunk,
            start: offset,
            end: offset + size,
        })
    }

    pub(super) fn alloc(
        &mut self,
        size: vk::DeviceSize,
        alignment: vk::DeviceSize,
    ) -> Option<Block> {
        let aligned_size = align(alignment, size);
        let block = (0..self.free.len())
            .find_map(|block| self.alloc_in(block, aligned_size, alignment))?;
        Some(block)
    }

    pub(super) fn free(&mut self, block: Block) {
        let chunk = block.chunk;
        let start = block.start;
        let end = block.end;

        self.used -= end - start;

        // Find insertion point
        // TODO: Binary search
        // TODO: If fragmentation is not an issue in practice, it might
        // not even be necessary to sort the free list
        let mut idx = self.free.len();
        for i in 0..self.free.len() {
            let block = self.free[i];
            if (block.chunk == chunk) & (start < block.start) {
                idx = i;
                break;
            }
        }

        // Detect adjacent blocks
        let merge_left = if idx > 0 {
            let left = self.free[idx - 1];
            assert!(left.chunk <= chunk);
            assert!((left.chunk < chunk) | (left.end <= start));
            (left.chunk == chunk) & (left.end == start)
        } else { false };
        let merge_right = if idx < self.free.len() {
            let right = self.free[idx];
            assert!(chunk <= right.chunk);
            assert!((chunk < right.chunk) | (end <= right.start));
            (right.chunk == chunk) & (end == right.start)
        } else { false };

        // Perform the insertion
        match (merge_left, merge_right) {
            (false, false) =>
                self.free.insert(idx, Block { chunk, start, end }),
            (true, false) => self.free[idx - 1].end = end,
            (false, true) => self.free[idx].start = start,
            (true, true) => {
                self.free[idx - 1].end = self.free[idx].end;
                self.free.remove(idx);
            },
        }
    }

    pub(super) fn clear(&mut self) {
        self.free.clear();
        self.used = 0;
        for (i, &size) in self.chunks.iter().enumerate() {
            self.free.push(Block {
                chunk: i as _,
                start: 0,
                end: size,
            });
        }
    }
}
