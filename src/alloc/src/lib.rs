#![feature(ptr_wrapping_offset_from)]
#![feature(trait_alias)]
use std::ptr;

mod bump;

pub use bump::*;

/// This trait bound is required so that data allocation is safe.
pub trait Pod = Sized + Copy;

/// Defines the memory requirements of an allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllocReqs {
    pub size: usize,
    pub alignment: usize,
}

impl AllocReqs {
    /// Returns the allocation requirements of a type according to the
    /// compiler.
    pub fn new<T>() -> Self {
        AllocReqs {
            size: std::mem::size_of::<T>(),
            alignment: std::mem::align_of::<T>(),
        }
    }
}

#[inline]
fn align_to(offset: usize, alignment: usize) -> usize {
    ((offset + alignment - 1) / alignment) * alignment
}

pub trait Allocator {
    unsafe fn alloc(&mut self, reqs: AllocReqs) -> *mut u8;

    #[inline]
    unsafe fn alloc_one<T: Pod>(&mut self) -> *mut T {
        self.alloc(AllocReqs {
            size: std::mem::size_of::<T>(),
            alignment: std::mem::align_of::<T>(),
        }) as _
    }

    #[inline]
    unsafe fn alloc_many<T: Pod>(&mut self, count: usize) -> *mut [T] {
        let ptr = self.alloc(AllocReqs {
            size: count * std::mem::size_of::<T>(),
            alignment: std::mem::align_of::<T>(),
        });
        std::slice::from_raw_parts_mut(ptr as *mut T, count) as _
    }

    #[inline]
    unsafe fn alloc_val<T: Pod>(&mut self, val: T) -> *mut T {
        let ptr = self.alloc_one::<T>();
        ptr.write(val);
        ptr
    }

    #[inline]
    unsafe fn alloc_slice<'a, T, A>(&mut self, slice: A) -> *mut [T]
    where
        A: AsRef<[T]>,
        T: Pod,
    {
        let slice = slice.as_ref();
        let ptr = self.alloc_many::<T>(slice.len());
        (*ptr).copy_from_slice(slice);
        ptr
    }

    #[inline]
    unsafe fn alloc_fill<'a, T: Pod>(&mut self, val: T, count: usize) ->
        *mut [T]
    {
        let slice = self.alloc_many::<T>(count);
        for elem in (*slice).iter_mut() { *elem = val; }
        slice
    }

    #[inline]
    unsafe fn alloc_iter<I>(&mut self, iter: I) ->
        *mut [<I as IntoIterator>::Item]
    where
        I: IntoIterator,
        <I as IntoIterator>::IntoIter: ExactSizeIterator,
        <I as IntoIterator>::Item: Pod,
    {
        let iter = iter.into_iter();
        let len = iter.len();
        let ptr = self.alloc_many::<<I as IntoIterator>::Item>(len);
        for (src, dst) in iter.zip((*ptr).iter_mut()) {
            ptr::write(dst, src);
        }
        ptr
    }
}

/// Like `Allocator::alloc_slice`, except returns a null pointer if the
/// slice is zero-sized.
#[inline]
pub unsafe fn alloc_slice_nonempty<A, S, T>(allocator: &mut A, slice: S)
    -> *mut [T]
where
    A: Allocator,
    S: AsRef<[T]>,
    T: Pod,
{
    let slice = slice.as_ref();
    let bytes = slice.len() * std::mem::size_of::<T>();
    if bytes > 0 { allocator.alloc_slice(slice) }
    else { std::slice::from_raw_parts_mut(ptr::null_mut(), 0) as _ }
}
