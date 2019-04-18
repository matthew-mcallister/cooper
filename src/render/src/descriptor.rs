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

/// An allocator for descriptor sets which can allocate but not free
/// sets. While Vulkan does support freeing descriptor sets with the
/// right flags set, it is generally superior to overwrite unused sets
/// than to free them.
#[derive(Debug)]
crate struct DescriptorSetAllocator {
    dt: Arc<vkl::DeviceTable>,
    layout: SetLayoutObj,
    pools: Vec<vk::DescriptorPool>,
    size: u32,
    capacity: u32,
}

impl Drop for DescriptorSetAllocator {
    fn drop(&mut self) {
        for &pool in self.pools.iter() {
            unsafe { self.dt.destroy_descriptor_pool(pool, ptr::null()); }
        }
    }
}

impl DescriptorSetAllocator {
    crate fn new(dt: Arc<vkl::DeviceTable>, layout: SetLayoutObj) -> Self {
        DescriptorSetAllocator {
            dt,
            layout,
            pools: Vec::new(),
            size: 0,
            capacity: 0,
        }
    }

    crate fn with_capacity(
        dt: Arc<vkl::DeviceTable>,
        layout: SetLayoutObj,
        cap: u32,
    ) -> Self {
        let mut res = DescriptorSetAllocator::new(dt, layout);
        res.new_pool(cap);
        res
    }

    crate fn size(&self) -> u32 {
        self.size
    }

    crate fn capacity(&self) -> u32 {
        self.capacity
    }

    fn grow_size(&self) -> u32 {
        (3 * self.capacity + 1) / 2
    }

    fn new_pool(&mut self, min_size: u32) {
        assert!(min_size > 0);
        let max_sets = std::cmp::max(min_size, self.grow_size());
        let sizes = self.layout.counts.pool_sizes(max_sets);
        let info = vk::DescriptorPoolCreateInfo {
            max_sets,
            pool_size_count: sizes.len() as _,
            p_pool_sizes: sizes.as_ptr(),
            ..Default::default()
        };
        unsafe {
            let mut pool = vk::null();
            self.dt.create_descriptor_pool
                (&info as _, ptr::null(), &mut pool as _).check().unwrap();
            self.pools.push(pool);
        }
    }

    fn do_alloc(&mut self, count: u32) {
        // Alloc from the top of the pool stack
        panic!("unimplemented");
    }
}
