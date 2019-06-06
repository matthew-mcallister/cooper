#![allow(dead_code)]
#![allow(unused_variables)]
use std::collections::BTreeMap;
use std::ptr;
use std::sync::Arc;

use crate::*;

#[derive(Clone, Debug)]
crate struct DescriptorCounts {
    crate counts: BTreeMap<vk::DescriptorType, u32>,
}

impl DescriptorCounts {
    crate fn pool_sizes(&self, multiplier: u32) -> Vec<vk::DescriptorPoolSize>
    {
        self.counts.iter()
            .map(|(&ty, &count)| vk::DescriptorPoolSize {
                ty,
                descriptor_count: count * multiplier,
            })
            .collect()
    }

    crate fn from_bindings(bindings: &[vk::DescriptorSetLayoutBinding]) -> Self
    {
        let mut counts = BTreeMap::new();
        for binding in bindings.iter() {
            let ty = binding.descriptor_type;
            let count = binding.descriptor_count;
            assert!(!counts.insert(ty, count).is_some());
        }
        DescriptorCounts { counts }
    }
}

/// An allocator for descriptor sets which creates the sets up front and
/// doles them out on request. Sets can even be returned to the pool
/// when they're no longer in use to be overwritten later.
///
/// # Safety
///
/// Freeing a descriptor set which was not allocated from this object
/// results in undefined behavior.
#[derive(Debug)]
crate struct DescriptorSetPool {
    dt: Arc<vkl::DeviceTable>,
    layout: SetLayoutObj,
    pools: Vec<vk::DescriptorPool>,
    sets: Vec<vk::DescriptorSet>,
    last_size: usize,
}

impl Drop for DescriptorSetPool {
    fn drop(&mut self) {
        for &pool in self.pools.iter() {
            unsafe { self.dt.destroy_descriptor_pool(pool, ptr::null()); }
        }
    }
}

impl DescriptorSetPool {
    crate fn new(
        dt: Arc<vkl::DeviceTable>,
        layout: SetLayoutObj,
    ) -> Self {
        DescriptorSetPool {
            dt,
            layout,
            pools: Vec::new(),
            free: Vec::new(),
            last_size: 0,
        }
    }

    crate fn with_capacity(
        dt: Arc<vkl::DeviceTable>,
        layout: SetLayoutObj,
        size: u32,
    ) -> Self {
        let mut res = DescriptorSetPool::new(dt, layout);
        res.new_pool(size);
        res
    }

    crate fn size(&self) -> usize {
        self.free.len()
    }

    fn grow_size(&self) -> u32 {
        (3 * self.last_size + 1) / 2
    }

    unsafe fn new_pool(&mut self, min_size: u32) {
        let min_size = if min_size == 0 { 16 } else { min_size };
        let max_sets = std::cmp::max(min_size, self.grow_size());
        self.last_size = max_sets;

        let sizes = self.layout.counts.pool_sizes(max_sets);
        let info = vk::DescriptorPoolCreateInfo {
            max_sets,
            pool_size_count: sizes.len() as _,
            p_pool_sizes: sizes.as_ptr(),
            ..Default::default()
        };
        let mut pool = vk::null();
        self.dt.create_descriptor_pool
            (&info as _, ptr::null(), &mut pool as _).check().unwrap();
        self.pools.push(pool);

        let set_layouts = vec![self.layout.obj; max_sets as usize];
        let info = vk::DescriptorSetAllocateInfo {
            descriptor_pool: pool,
            descriptor_set_count: max_sets,
            p_set_layouts: set_layouts.as_ptr(),
            ..Default::default()
        };
        let old_len = self.free.len();
        let new_len = old_len + max_sets as usize;
        self.free.resize(new_len, vk::null());
        self.dt.allocate_descriptor_sets
            (&info as _, self.free[old_len..].as_mut_ptr());
    }

    /// Guarantees that at least `additional` free sets are available.
    crate unsafe fn reserve(&mut self, additional: usize) {
        let new = additional as isize - self.free.len() as isize;
        if new > 0 { self.new_pool(new as _); }
    }

    crate unsafe fn allocate(&mut self) -> vk::DescriptorSet {
        self.reserve(1);
        self.free.pop()
    }

    crate fn free(&mut self, desc: vk::DescriptorSet) {
        self.free.push(desc);
    }
}
