use crate::{AllocReqs, Allocator, align_to};

/// A bump allocator allocates memory by bumping a pointer. It can only
/// free memory all in one go by destroying the allocator. It's a really
/// easy automatic memory management alternative to RAII.
///
/// Caveats: the current implementation will waste space whenever the
/// current capacity is overflowed and whenever the next allocation
/// position is adjusted for alignment. A better allocator would
/// probably pool allocations by size.
#[derive(Debug)]
pub struct BumpAllocator {
    chunks: Vec<Box<[u8]>>,
    offset: usize,
}

impl BumpAllocator {
    pub fn with_capacity(capacity: usize) -> Self {
        let mut res = BumpAllocator {
            chunks: Vec::new(),
            offset: 0,
        };
        res.new_chunk(capacity);
        res
    }

    fn new_chunk(&mut self, size: usize) {
        let mut vec = Vec::with_capacity(size);
        unsafe { vec.set_len(size); }
        self.chunks.push(vec.into());
    }

    #[inline]
    pub fn remaining_capacity(&self) -> usize {
        self.chunks.last().unwrap().len() - self.offset
    }
}

impl Allocator for BumpAllocator {
    unsafe fn alloc(&mut self, reqs: AllocReqs) -> *mut u8 {
        let mut offset = align_to(self.offset, reqs.alignment);

        let last_alloc_size = self.chunks.last().unwrap().len();
        if offset + reqs.size > last_alloc_size {
            let grow_size = 3 * (last_alloc_size + 1) / 2;
            let chunk_size = std::cmp::max(reqs.size, grow_size);
            self.new_chunk(chunk_size);
            offset = 0;
        }

        let res = &mut self.chunks.last_mut().unwrap()[offset] as _;
        self.offset = offset + reqs.size;
        res
    }
}

#[cfg(test)]
mod test {
    use crate::{Allocator, AllocReqs};
    use super::*;

    #[test]
    fn test_single_chunk() {
        let mut allocator = BumpAllocator::with_capacity(32);
        unsafe {
            let ptr1 = allocator.alloc(AllocReqs { size: 12, alignment: 4 });
            let ptr2 = allocator.alloc(AllocReqs { size: 8, alignment: 8 });
        }
        assert_eq!(ptr2.wrapping_offset_from(ptr1), 16);
        assert_eq!(allocator.remaining_capacity(), 8);
    }

    #[test]
    fn test_multi_chunk() {
        let mut allocator = BumpAllocator::with_capacity(32);
        allocator.alloc(AllocReqs { size: 32, alignment: 8 });
        assert_eq!(allocator.remaining_capacity(), 0);
        unsafe {
            let ptr1 = allocator.alloc(AllocReqs { size: 12, alignment: 4 });
            let ptr2 = allocator.alloc(AllocReqs { size: 8, alignment: 8 });
        }
        assert_eq!(ptr2.wrapping_offset_from(ptr1), 16);
    }

    #[test]
    fn test_alloc_one() {
        let mut allocator = BumpAllocator::with_capacity(32);
        unsafe {
            let ptr1 = allocator.alloc_one::<u32>();
            let ptr2 = allocator.alloc_one::<u32>();
        }
        assert_eq!
            ((ptr2 as *const u8).wrapping_offset_from(ptr1 as *const u8), 4);
        assert_eq!(allocator.remaining_capacity(), 24);
    }

    #[test]
    fn test_alloc_many() {
        let mut allocator = BumpAllocator::with_capacity(32);
        unsafe {
            let ptr1 = allocator.alloc_many::<u32>(3);
            assert_eq!((*ptr1).len(), 3);
            let ptr2 = allocator.alloc_one::<u32>();
        }
        assert_eq!
            ((ptr2 as *const u8).wrapping_offset_from(ptr1 as *const u8), 12);
        assert_eq!(allocator.remaining_capacity(), 16);
    }

    #[test]
    fn test_alloc_val() {
        let mut allocator = BumpAllocator::with_capacity(32);
        unsafe {
            let ptr1 = allocator.alloc_val(2u32);
            let ptr2 = allocator.alloc_val(3u32);
            assert_eq!(*ptr2 - *ptr1, 1);
        }
        assert_eq!
            ((ptr2 as *const u8).wrapping_offset_from(ptr1 as *const u8), 4);
        assert_eq!(allocator.remaining_capacity(), 24);
    }

    #[test]
    fn test_alloc_slice() {
        let mut allocator = BumpAllocator::with_capacity(32);
        let data = [0u8, 1, 2, 3];
        unsafe {
            let ptr1 = allocator.alloc_slice(&data);
            assert_eq!(&(*ptr1), &data);
        }
        let ptr2 = allocator.alloc_val(0u32);
        assert_eq!
            ((ptr2 as *const u8).wrapping_offset_from(ptr1 as *const u8), 4);
        assert_eq!(allocator.remaining_capacity(), 24);
    }

    #[test]
    fn test_alloc_fill() {
        let mut allocator = BumpAllocator::with_capacity(32);
        unsafe {
            let ptr = allocator.alloc_fill(1u8, 4);
            assert_eq!(&(*ptr), &[1u8; 4]);
        }
        assert_eq!(allocator.remaining_capacity(), 28);
    }

    #[test]
    fn test_alloc_iter() {
        let mut allocator = BumpAllocator::with_capacity(32);
        unsafe {
            let ptr = allocator.alloc_iter([1u8, 2, 3, 4].iter().cloned());
            assert_eq!(&(*ptr), &[1, 2, 3, 4]);
        }
        assert_eq!(allocator.remaining_capacity(), 28);
    }
}
