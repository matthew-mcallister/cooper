use std::ptr;
use std::sync::Arc;

use base::{HashVector, Sentinel};
use fnv::FnvHashMap;

use crate::*;

// Sorry if this causes more confusion than good
use self::{
    DescriptorCounts as Counts, DescriptorPool as Pool,
    DescriptorSet as Set, DescriptorSetLayout as Layout,
};

type DescriptorCounts = HashVector<vk::DescriptorType, u32>;

crate fn pool_sizes(counts: &DescriptorCounts) -> Vec<vk::DescriptorPoolSize> {
    counts.iter()
        .map(|(&ty, &descriptor_count)| {
            vk::DescriptorPoolSize { ty, descriptor_count }
        })
        .collect()
}

crate fn count_descriptors(bindings: &[vk::DescriptorSetLayoutBinding]) ->
    Counts
{
    bindings.iter()
        .map(|binding| (binding.descriptor_type, binding.descriptor_count))
        .sum()
}

/// Returns a reasonable number of descriptor sets and pool sizes for
/// a global descriptor pool.
crate fn global_descriptor_counts() -> (u32, Counts) {
    let max_sets = 0x1_0000;
    let max_descs = [
        (vk::DescriptorType::SAMPLER,                   1 * max_sets),
        (vk::DescriptorType::COMBINED_IMAGE_SAMPLER,    8 * max_sets),
        (vk::DescriptorType::SAMPLED_IMAGE,             8 * max_sets),
        (vk::DescriptorType::STORAGE_IMAGE,             1 * max_sets),
        (vk::DescriptorType::UNIFORM_TEXEL_BUFFER,      1 * max_sets),
        (vk::DescriptorType::STORAGE_TEXEL_BUFFER,      1 * max_sets),
        (vk::DescriptorType::UNIFORM_BUFFER,            1 * max_sets),
        (vk::DescriptorType::STORAGE_BUFFER,            1 * max_sets),
        (vk::DescriptorType::INPUT_ATTACHMENT,          256),
    ].iter().cloned().collect();
    (max_sets, max_descs)
}

crate unsafe fn create_global_pool(device: Arc<Device>) -> DescriptorPool {
    let (max_sets, max_descriptors) = global_descriptor_counts();
    let pool_sizes = pool_sizes(&max_descriptors);
    let create_info = vk::DescriptorPoolCreateInfo {
        flags: vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET_BIT,
        max_sets,
        pool_size_count: pool_sizes.len() as _,
        p_pool_sizes: pool_sizes.as_ptr(),
        ..Default::default()
    };
    DescriptorPool::new(device, &create_info)
}

#[derive(Clone, Debug)]
crate struct DescriptorSetLayout {
    device: Arc<Device>,
    inner: vk::DescriptorSetLayout,
    flags: vk::DescriptorSetLayoutCreateFlags,
    counts: Counts,
}

impl Drop for DescriptorSetLayout {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe { dt.destroy_descriptor_set_layout(self.inner, ptr::null()); }
    }
}

impl Layout {
    crate unsafe fn new(
        device: Arc<Device>,
        create_info: &vk::DescriptorSetLayoutCreateInfo,
    ) -> Self {
        create_descriptor_set_layout(device, &create_info)
    }

    crate unsafe fn from_bindings(
        device: Arc<Device>,
        flags: vk::DescriptorSetLayoutCreateFlags,
        bindings: &[vk::DescriptorSetLayoutBinding],
    ) -> Self {
        let create_info = vk::DescriptorSetLayoutCreateInfo {
            flags,
            binding_count: bindings.len() as _,
            p_bindings: bindings.as_ptr(),
            ..Default::default()
        };
        Self::new(device, &create_info)
    }

    crate fn inner(&self) -> vk::DescriptorSetLayout {
        self.inner
    }

    crate fn flags(&self) -> vk::DescriptorSetLayoutCreateFlags {
        self.flags
    }

    crate fn counts(&self) -> &Counts {
        &self.counts
    }

    crate fn required_pool_flags(&self) -> vk::DescriptorPoolCreateFlags {
        let mut flags = Default::default();

        let update_after_bind =
            vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL_BIT_EXT;
        if self.flags.contains(update_after_bind) {
            flags |= vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND_BIT_EXT;
        }

        flags
    }
}

unsafe fn create_descriptor_set_layout(
    device: Arc<Device>,
    create_info: &vk::DescriptorSetLayoutCreateInfo,
) -> DescriptorSetLayout {
    let dt = &*device.table;
    let bindings = std::slice::from_raw_parts(
        create_info.p_bindings,
        create_info.binding_count as _,
    );
    let counts = count_descriptors(bindings);
    let flags = create_info.flags;
    let mut inner = vk::null();
    dt.create_descriptor_set_layout(create_info, ptr::null(), &mut inner)
        .check().unwrap();
    DescriptorSetLayout {
        device,
        inner,
        flags,
        counts,
    }
}

#[derive(Debug)]
crate struct DescriptorSet {
    layout: Arc<DescriptorSetLayout>,
    sentinel: Sentinel,
    inner: vk::DescriptorSet,
}

impl DescriptorSet {
    crate fn inner(&self) -> vk::DescriptorSet {
        self.inner
    }

    crate fn layout(&self) -> &Arc<DescriptorSetLayout> {
        &self.layout
    }
}

/// Fixed-size general-purpose descriptor pool.
#[derive(Debug)]
crate struct DescriptorPool {
    device: Arc<Device>,
    inner: vk::DescriptorPool,
    flags: vk::DescriptorPoolCreateFlags,
    // Provides a reference count for safety
    sentinel: Sentinel,
    // Note: limits are not hard but are informative
    max_sets: u32,
    used_sets: u32,
    max_descriptors: Counts,
    used_descriptors: Counts,
}

impl Drop for Pool {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        assert!(!self.sentinel.in_use());
        unsafe { dt.destroy_descriptor_pool(self.inner, ptr::null()); }
    }
}

impl Pool {
    crate unsafe fn new(
        device: Arc<Device>,
        create_info: &vk::DescriptorPoolCreateInfo,
    ) -> Self {
        let dt = &*device.table;

        assert!(create_info.max_sets > 0);
        let pool_sizes = std::slice::from_raw_parts(
            create_info.p_pool_sizes,
            create_info.pool_size_count as usize,
        );
        let max_descriptors = pool_sizes.iter()
            .map(|pool_size| (pool_size.ty, pool_size.descriptor_count))
            .collect();

        let mut inner = vk::null();
        dt.create_descriptor_pool(create_info, ptr::null(), &mut inner)
            .check().unwrap();

        DescriptorPool {
            device,
            inner,
            flags: create_info.flags,
            sentinel: Sentinel::new(),
            max_sets: create_info.max_sets,
            used_sets: 0,
            max_descriptors,
            used_descriptors: Default::default(),
        }
    }

    crate fn can_free(&self) -> bool {
        let flag = vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET_BIT;
        self.flags.contains(flag)
    }

    crate fn max_sets(&self) -> u32 {
        self.max_sets
    }

    crate fn used_sets(&self) -> u32 {
        self.used_sets
    }

    crate fn max_descriptors(&self) -> &Counts {
        &self.max_descriptors
    }

    crate fn used_descriptors(&self) -> &Counts {
        &self.used_descriptors
    }

    crate unsafe fn alloc_many(
        &mut self,
        layout: &Arc<DescriptorSetLayout>,
        count: u32,
    ) -> Vec<DescriptorSet> {
        assert!(self.flags.contains(layout.required_pool_flags()));

        self.used_sets += count;
        self.used_descriptors += layout.counts() * count;

        // XXX: use smallvec here
        let dt = &*self.device.table;
        let mut sets = vec![vk::null(); count as usize];
        let layouts = vec![layout.inner(); count as usize];
        let alloc_info = vk::DescriptorSetAllocateInfo {
            descriptor_pool: self.inner,
            descriptor_set_count: count,
            p_set_layouts: layouts.as_ptr(),
            ..Default::default()
        };
        dt.allocate_descriptor_sets(&alloc_info, sets.as_mut_ptr())
            .check().unwrap();

        sets.into_iter().map(|inner| {
            DescriptorSet {
                layout: layout.clone(),
                sentinel: self.sentinel.clone(),
                inner,
            }
        }).collect()
    }

    crate unsafe fn alloc(&mut self, layout: &Arc<DescriptorSetLayout>) -> Set
    {
        self.alloc_many(layout, 1).pop().unwrap()
    }

    crate unsafe fn free(&mut self, set: Set) {
        assert!(self.can_free());
        assert_eq!(self.sentinel, set.sentinel);

        self.used_sets -= 1;
        self.used_descriptors -= set.layout.counts();

        let dt = &*self.device.table;
        let sets = std::slice::from_ref(&set.inner);
        dt.free_descriptor_sets(self.inner, sets.len() as _, sets.as_ptr())
            .check().unwrap();
    }

    crate unsafe fn reset(&mut self) {
        assert!(!self.sentinel.in_use());
        let dt = &*self.device.table;
        dt.reset_descriptor_pool(self.inner, Default::default());
    }
}

#[cfg(test)]
mod tests {
    use vk::traits::*;
    use base::Name;
    use super::*;

    unsafe fn create_test_set_layouts(device: &Arc<Device>) ->
        fnv::FnvHashMap<Name, Arc<DescriptorSetLayout>>
    {
        let mut map = FnvHashMap::default();

        let flags = Default::default();

        let bindings = [
            vk::DescriptorSetLayoutBinding {
                binding: 0,
                descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::VERTEX_BIT
                    | vk::ShaderStageFlags::FRAGMENT_BIT,
                ..Default::default()
            },
            vk::DescriptorSetLayoutBinding {
                binding: 1,
                descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::FRAGMENT_BIT,
                ..Default::default()
            },
        ];
        map.insert(
            Name::new("scene_globals"),
            Arc::new(DescriptorSetLayout::from_bindings(
                Arc::clone(&device), flags, &bindings,
            )),
        );

        let bindings = [vk::DescriptorSetLayoutBinding {
            binding: 0,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 3,
            stage_flags: vk::ShaderStageFlags::FRAGMENT_BIT,
            ..Default::default()
        }];
        map.insert(
            Name::new("material"),
            Arc::new(DescriptorSetLayout::from_bindings(
                Arc::clone(&device), flags, &bindings,
            )),
        );

        map
    }

    unsafe fn smoke_test(vars: testing::TestVars) {
        let device = Arc::clone(&vars.swapchain.device);

        let layouts = create_test_set_layouts(&device);
        let mut pool = create_global_pool(device);

        let scene_global_layout = &layouts[&Name::new("scene_globals")];
        let material_layout = &layouts[&Name::new("material")];

        let set0 = pool.alloc(scene_global_layout);
        let sets = pool.alloc_many(material_layout, 3);

        let used = pool.used_descriptors();
        assert_eq!(pool.used_sets(), 4);
        assert_eq!(used.get(&vk::DescriptorType::STORAGE_BUFFER), 2);
        assert_eq!(used.get(&vk::DescriptorType::COMBINED_IMAGE_SAMPLER), 9);

        assert!(!sets.iter().any(|set| set.inner.is_null()));
        assert_ne!(sets[0].inner, sets[1].inner);
        assert_ne!(sets[1].inner, sets[2].inner);
        assert_ne!(sets[2].inner, sets[0].inner);

        pool.free(set0);
        let used = pool.used_descriptors();
        assert_eq!(pool.used_sets(), 3);
        assert_eq!(used.get(&vk::DescriptorType::STORAGE_BUFFER), 0);
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
