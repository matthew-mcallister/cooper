use std::ops::Range;

use prelude::*;

use super::*;

pub(super) trait Allocator: Default {
    // TODO: Likely want to record two numbers: true memory usage and
    // (approximate) memory usage including fragmentation.
    //   true_usage = sum(alloc.size for each successful alloc)
    //   frag_usage = capacity - amount available for allocation
    // TODO: This logic shouldn't need to be reimplemented for every
    // implementor of the Allocator trait.
    fn used(&self) -> vk::DeviceSize;
    fn capacity(&self) -> vk::DeviceSize;
    fn add_chunk(&mut self, size: vk::DeviceSize);
    fn alloc(&mut self, size: vk::DeviceSize, alignment: vk::DeviceSize) ->
        Option<Block>;
    fn free(&mut self, block: Block);
    fn clear(&mut self);
}

/// Address-ordered FIFO allocation algorithm.
#[derive(Debug, Default)]
pub(super) struct FreeListAllocator {
    used: vk::DeviceSize,
    // List of chunk sizes
    chunks: Vec<vk::DeviceSize>,
    free: Vec<Block>,
}

impl FreeListAllocator {
    pub(super) fn new() -> Self {
        Default::default()
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

    fn alloc_in(
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

    pub(super) fn do_free(&mut self, block: Block) {
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
}

impl Allocator for FreeListAllocator {
    fn used(&self) -> vk::DeviceSize {
        self.used
    }

    fn capacity(&self) -> vk::DeviceSize {
        self.chunks.iter().sum()
    }

    fn add_chunk(&mut self, size: vk::DeviceSize) {
        self.chunks.push(size);
        self.free.push(Block {
            chunk: (self.chunks.len() - 1) as _,
            start: 0,
            end: size,
        });
    }

    fn alloc(
        &mut self,
        size: vk::DeviceSize,
        alignment: vk::DeviceSize,
    ) -> Option<Block> {
        let aligned_size = align(alignment, size);
        let block = (0..self.free.len())
            .find_map(|block| self.alloc_in(block, aligned_size, alignment))?;
        Some(block)
    }

    fn free(&mut self, block: Block) {
        self.do_free(block);
    }

    fn clear(&mut self) {
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

/// Allocator that works by bumping a pointer. It can only free all used
/// memory at one time.
#[derive(Debug, Default)]
pub(super) struct LinearAllocator {
    // List of chunk sizes
    chunks: Vec<vk::DeviceSize>,
    // Current chunk
    chunk: usize,
    // Offset into current chunk
    offset: vk::DeviceSize,
}

impl LinearAllocator {
    pub(super) fn new() -> Self {
        Default::default()
    }

    fn alloc_in(
        &mut self,
        size: vk::DeviceSize,
        alignment: vk::DeviceSize,
    ) -> Option<Block> {
        let start = align(alignment, self.offset);
        let end = self.offset + align(alignment, size);
        (end <= *self.chunks.get(self.chunk)?).then(|| {
            self.offset = end;
            Block {
                chunk: self.chunk as _,
                start,
                end,
            }
        })
    }

    fn next_chunk(&mut self) -> Option<()> {
        (self.chunk + 1 < self.chunks.len()).then(|| {
            self.chunk += 1;
            self.offset = 0;
        })
    }
}

impl Allocator for LinearAllocator {
    fn used(&self) -> vk::DeviceSize {
        // TODO: Why doesn't the LHS type resolve?
        self.chunks[..self.chunk].iter().sum(): vk::DeviceSize + self.offset
    }

    fn capacity(&self) -> vk::DeviceSize {
        self.chunks.iter().sum()
    }

    fn add_chunk(&mut self, size: vk::DeviceSize) {
        self.chunks.push(size);
    }

    fn alloc(
        &mut self,
        size: vk::DeviceSize,
        alignment: vk::DeviceSize,
    ) -> Option<Block> {
        self.alloc_in(size, alignment).or_else(|| {
            // TODO: possibly refine strategy for very large requests
            self.next_chunk()?;
            self.alloc_in(size, alignment)
        })
    }

    fn free(&mut self, _: Block) {}

    fn clear(&mut self) {
        self.chunk = 0;
        self.offset = 0;
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use super::*;

    fn linear_inner(alloc: &mut LinearAllocator) {
        assert_eq!(alloc.used(), 0);
        assert_eq!(alloc.capacity(), 2048);

        // Alignment
        assert_eq!(alloc.alloc(4, 8), Some(Block {
            chunk: 0,
            start: 0,
            end: 8,
        }));
        assert_eq!(alloc.alloc(4, 8), Some(Block {
            chunk: 0,
            start: 8,
            end: 16,
        }));
        assert_eq!(alloc.used(), 16);
        assert_eq!(alloc.capacity(), 2048);

        // Free is no-op
        alloc.free(Block { chunk: 0, start: 0, end: 16, });
        assert_eq!(alloc.used(), 16);
        assert_eq!(alloc.capacity(), 2048);

        // Spill over to next chunk
        assert_eq!(alloc.alloc(1000, 8), Some(Block {
            chunk: 0,
            start: 16,
            end: 1016,
        }));
        assert_eq!(alloc.alloc(64, 8), Some(Block {
            chunk: 1,
            start: 0,
            end: 64,
        }));
        assert_eq!(alloc.used(), 1088);
        assert_eq!(alloc.capacity(), 2048);

        // Cannot alloc past the end of the chunk
        assert_eq!(alloc.alloc(1000, 8), None);
        assert_eq!(alloc.used(), 1088);
        assert_eq!(alloc.capacity(), 2048);

        // Can alloc to end of chunk
        assert_eq!(alloc.alloc(960, 8), Some(Block {
            chunk: 1,
            start: 64,
            end: 1024,
        }));
        assert_eq!(alloc.used(), alloc.capacity());

        assert_eq!(alloc.alloc(8, 8), None);
    }

    fn linear(_: testing::TestVars) {
        let mut alloc = LinearAllocator::new();

        alloc.add_chunk(1024);
        alloc.add_chunk(1024);

        // Run test, clear, and run it again
        linear_inner(&mut alloc);
        alloc.clear();
        linear_inner(&mut alloc);
    }

    unit::declare_tests![linear];
}

unit::collect_tests![tests];
