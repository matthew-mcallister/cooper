use std::collections::BTreeMap;

use crate::*;

macro_rules! insert_unique {
    ($map:expr; $key:expr => $val:expr) => {
        assert!(!$map.insert($key, $val).is_some());
    }
}

#[derive(Clone, Debug)]
pub struct DescriptorCounts {
    pub inner: BTreeMap<vk::DescriptorType, u32>,
}

impl DescriptorCounts {
    pub fn pool_sizes(&self, multiplier: u32) -> Vec<vk::DescriptorPoolSize>
    {
        self.inner.iter()
            .map(|(&ty, &count)| vk::DescriptorPoolSize {
                ty,
                descriptor_count: count * multiplier,
            })
            .collect()
    }

    pub fn from_bindings(bindings: &[vk::DescriptorSetLayoutBinding]) -> Self
    {
        let mut inner = BTreeMap::new();
        for binding in bindings.iter() {
            let ty = binding.descriptor_type;
            let count = binding.descriptor_count;
            insert_unique!(inner; ty => count);
        }
        DescriptorCounts { inner }
    }
}

#[derive(Clone, Debug)]
pub struct SetLayoutInfo {
    pub inner: vk::DescriptorSetLayout,
    pub counts: DescriptorCounts,
}

crate unsafe fn create_descriptor_set_layout(
    objs: &mut ObjectTracker,
    bindings: &[vk::DescriptorSetLayoutBinding],
) -> SetLayoutInfo {
    let create_info = vk::DescriptorSetLayoutCreateInfo {
        binding_count: bindings.len() as _,
        p_bindings: bindings.as_ptr(),
        ..Default::default()
    };
    let layout = objs.create_descriptor_set_layout(&create_info);
    let counts = DescriptorCounts::from_bindings(&bindings);
    SetLayoutInfo {
        inner: layout,
        counts,
    }
}

#[derive(Clone, Copy, Debug, Default)]
crate struct CreateDescriptorSetParams {
    crate count: u32,
    crate pool_flags: vk::DescriptorPoolCreateFlags,
}

crate unsafe fn create_descriptor_sets(
    objs: &mut ObjectTracker,
    set_layout: &SetLayoutInfo,
    params: CreateDescriptorSetParams,
) -> (vk::DescriptorPool, Vec<vk::DescriptorSet>) {
    let pool_sizes = set_layout.counts.pool_sizes(params.count);
    let create_info = vk::DescriptorPoolCreateInfo {
        max_sets: params.count,
        pool_size_count: pool_sizes.len() as _,
        p_pool_sizes: pool_sizes.as_ptr(),
        ..Default::default()
    };
    let descriptor_pool = objs.create_descriptor_pool(&create_info);

    let mut sets = vec![vk::null(); params.count as usize];
    let layouts = vec![set_layout.inner; params.count as usize];
    let alloc_info = vk::DescriptorSetAllocateInfo {
        descriptor_pool,
        descriptor_set_count: params.count,
        p_set_layouts: layouts.as_ptr(),
        ..Default::default()
    };
    objs.device.table.allocate_descriptor_sets
        (&alloc_info as _, sets.as_mut_ptr()).check().unwrap();

    (descriptor_pool, sets)
}
