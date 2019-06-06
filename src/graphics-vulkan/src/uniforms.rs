use std::convert::TryFrom;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr;

use alloc::Pod
use derive_more::*;
use slab::Slab;

use crate::*;

pub const CHAIN_BUFFER_COUNT: usize = 2;

/// A set of same-sized, memory-mapped, uniform buffers that can be
/// swapped out as multiple frames are in flight.
#[derive(Debug)]
pub struct UniformBufferChain {
    buffers: [UniformBuffer; BUFFER_COUNT],
}

#[derive(Debug)]
struct UniformBuffer {
    buffer: vk::Buffer,
    memory: CommonAlloc,
    user: u64,
}

#[derive(Debug)]
pub struct UniformBufferHandle {
    pub slice: *mut [u8],
    pub user: u64,
}

impl UniformBufferChain {
    pub fn available(&self, buffer: usize) -> bool {
    }

    pub fn try_acquire(&self, buffer: usize) -> Option<UniformBufferHandle> {
    }

    pub fn wait_for(&self, buffer: usize) -> UniformBufferHandle {
    }
}
