use std::ptr;
use std::sync::Arc;

use ccore::name::*;
use fnv::FnvHashMap;
use parking_lot::{Mutex, RwLock};

use crate::*;

// Sorry if this causes more confusion than good
use self::{
    DescriptorAllocator as Allocator, DescriptorCounts as Counts,
    DescriptorSet as Set, DescriptorSetLayout as Layout,
};

/// Maps descriptor type to descriptor count for a set layout.
#[derive(Clone, Debug)]
crate struct DescriptorCounts {
    crate inner: FnvHashMap<vk::DescriptorType, u32>,
}

impl Counts {
    crate fn pool_sizes(&self, multiplier: u32) -> Vec<vk::DescriptorPoolSize> {
        self.inner.iter()
            .map(|(&ty, &count)| vk::DescriptorPoolSize {
                ty,
                descriptor_count: count * multiplier,
            })
            .collect()
    }

    crate fn from_bindings(bindings: &[vk::DescriptorSetLayoutBinding]) -> Self {
        let mut inner = FnvHashMap::default();
        for binding in bindings.iter() {
            let ty = binding.descriptor_type;
            let count = binding.descriptor_count;
            *inner.entry(ty).or_insert(0) += count;
        }
        Counts { inner }
    }
}

/// Tells the allocator how to allocate sets of a particular layout.
#[derive(Clone, Copy, Debug)]
crate struct DescriptorSetAllocPolicy {
    crate pool_size: u32,
}

impl Default for DescriptorSetAllocPolicy {
    fn default() -> Self {
        DescriptorSetAllocPolicy {
            pool_size: 256,
        }
    }
}

#[derive(Clone, Debug)]
crate struct DescriptorSetLayout {
    inner: vk::DescriptorSetLayout,
    flags: vk::DescriptorSetLayoutCreateFlags,
    counts: Counts,
    alloc_policy: DescriptorSetAllocPolicy,
}

impl Layout {
    crate fn inner(&self) -> vk::DescriptorSetLayout {
        self.inner
    }

    crate fn flags(&self) -> vk::DescriptorSetLayoutCreateFlags {
        self.flags
    }

    crate fn counts(&self) -> &Counts {
        &self.counts
    }

    crate fn alloc_policy(&self) -> &DescriptorSetAllocPolicy {
        &self.alloc_policy
    }

    crate fn pool_flags(&self) -> vk::DescriptorPoolCreateFlags {
        let mut flags = Default::default();

        let update_after_bind =
            vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL_BIT_EXT;
        if self.flags.contains(update_after_bind) {
            flags |= vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND_BIT_EXT;
        }

        flags
    }

    unsafe fn create_pool(&self, device: &Device, size: u32) ->
        vk::DescriptorPool
    {
        create_pool(device, self, size)
    }
}

crate unsafe fn create_descriptor_set_layout(
    device: &Device,
    create_info: &vk::DescriptorSetLayoutCreateInfo,
    alloc_policy: DescriptorSetAllocPolicy,
) -> DescriptorSetLayout {
    let dt = &*device.table;
    let bindings = std::slice::from_raw_parts(
        create_info.p_bindings,
        create_info.binding_count as _,
    );
    let counts = Counts::from_bindings(bindings);
    let flags = create_info.flags;
    let mut inner = vk::null();
    dt.create_descriptor_set_layout(create_info, ptr::null(), &mut inner)
        .check().unwrap();
    DescriptorSetLayout {
        inner,
        flags,
        counts,
        alloc_policy,
    }
}

unsafe fn create_pool(device: &Device, layout: &Layout, size: u32) ->
    vk::DescriptorPool
{
    let dt = &device.table;

    let pool_sizes = layout.counts.pool_sizes(size);
    let flags = layout.pool_flags();

    let create_info = vk::DescriptorPoolCreateInfo {
        flags,
        max_sets: size,
        pool_size_count: pool_sizes.len() as _,
        p_pool_sizes: pool_sizes.as_ptr(),
        ..Default::default()
    };
    let mut inner = vk::null();
    dt.create_descriptor_pool(&create_info, ptr::null(), &mut inner)
        .check().unwrap();

    inner
}

#[derive(Debug)]
crate struct DescriptorSet {
    inner: vk::DescriptorSet,
    key: Name,
    pool: usize,
}

impl DescriptorSet {
    crate fn inner(&self) -> vk::DescriptorSet {
        self.inner
    }

    crate fn layout(&self) -> Name {
        self.key
    }
}

#[derive(Debug)]
struct Subpool {
    inner: vk::DescriptorPool,
    size: u32,
}

impl Subpool {
    unsafe fn alloc_sets(
        &self,
        device: &Device,
        layout: vk::DescriptorSetLayout,
    ) -> Vec<vk::DescriptorSet> {
        let dt = &*device.table;
        let count = self.size;

        let mut sets = vec![vk::null(); count as usize];
        let layouts = vec![layout; count as usize];
        let alloc_info = vk::DescriptorSetAllocateInfo {
            descriptor_pool: self.inner,
            descriptor_set_count: count,
            p_set_layouts: layouts.as_ptr(),
            ..Default::default()
        };
        dt.allocate_descriptor_sets(&alloc_info, sets.as_mut_ptr())
            .check().unwrap();
        sets
    }
}

/// Acts like a dynamically growing descriptor pool.
/// TODO: It would be more efficient to separate pools from layouts.
#[derive(Debug)]
struct Suballocator {
    device: Arc<Device>,
    free: Vec<Set>,
    sub_pools: Vec<Subpool>,
    layout: *const Layout,
    key: Name,
}

impl Suballocator {
    unsafe fn grow_by(&mut self, size: u32) {
        assert!(size > 0);
        let device = &*self.device;
        let layout = &*self.layout; // raw deref here
        let key = self.key;

        // Add new descriptor pool
        let pool_inner = layout.create_pool(device, size);
        let pool = Subpool { inner: pool_inner, size };

        // Allocate all possible sets
        let sets = pool.alloc_sets(device, layout.inner);

        // Add to free list
        let pool_idx = self.sub_pools.len();
        let sets = sets.into_iter()
            .map(|inner| Set {
                inner,
                key,
                pool: pool_idx,
            });
        self.free.extend(sets);

        // Save the pool object
        self.sub_pools.push(pool);
    }

    unsafe fn grow(&mut self) {
        let layout = &*self.layout;
        let size = layout.alloc_policy.pool_size;
        self.grow_by(size);
    }

    unsafe fn allocate(&mut self) -> Set {
        if let Some(set) = self.free.pop() {
            return set;
        }

        self.grow();
        self.free.pop().unwrap()
    }

    fn free(&mut self, set: Set) {
        assert_eq!(set.key, self.key);
        self.free.push(set);
    }
}

#[derive(Debug)]
crate struct DescriptorAllocator {
    device: Arc<Device>,
    // TODO: RwLock + HashMap + Mutex is seemingly inferior to a true
    // concurrent hash map. Worth switching to?
    sub_alloc: RwLock<FnvHashMap<Name, Mutex<Suballocator>>>,
}

unsafe impl Send for DescriptorAllocator {}
unsafe impl Sync for DescriptorAllocator {}

impl Drop for Allocator {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            for sub in self.sub_alloc.get_mut().values_mut() {
                for pool in sub.get_mut().sub_pools.iter() {
                    dt.destroy_descriptor_pool(pool.inner, ptr::null());
                }
            }
        }
    }
}

impl Allocator {
    crate fn new(device: Arc<Device>) -> Self {
        Allocator {
            device,
            sub_alloc: RwLock::new(Default::default()),
        }
    }

    crate unsafe fn allocate(&self, layout: Name) -> Option<Set> {
        Some(self.sub_alloc.read()
            .get(&layout)?
            .lock()
            .allocate())
    }

    crate unsafe fn insert_alloc(&self, key: Name, layout: *const Layout) ->
        Set
    {
        self.sub_alloc.write()
            .entry(key)
            .or_insert_with(|| {
                Mutex::new(Suballocator {
                    device: Arc::clone(&self.device),
                    free: Vec::new(),
                    sub_pools: Vec::new(),
                    layout,
                    key,
                })
            })
            .get_mut()
            .allocate()
    }

    crate fn free(&self, set: Set) {
        use std::ops::Index;
        self.sub_alloc.read()
            .index(&set.key)
            .lock()
            .free(set.into());
    }
}

#[cfg(test)]
mod tests {
    use std::thread;
    use ccore::name::*;
    use vk::traits::*;
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let device = Arc::clone(&vars.swapchain.device);
        let core_data = Arc::new(CoreData::new(device));

        let core = Arc::clone(&core_data);
        let thread1 = thread::spawn(move || {
            let layout = Name::new("scene_globals");
            let set0 = core.alloc_desc_set(layout);
            let sets = [
                core.alloc_desc_set(layout),
                core.alloc_desc_set(layout),
                core.alloc_desc_set(layout),
            ];
            core.free_desc_set(set0);

            assert!(!sets.iter().any(|set| set.inner.is_null()));
            assert_ne!(sets[0].inner, sets[1].inner);
            assert_ne!(sets[1].inner, sets[2].inner);
            assert_ne!(sets[2].inner, sets[0].inner);
        });

        let core = Arc::clone(&core_data);
        let thread2 = thread::spawn(move || {
            let set = core.alloc_desc_set(Name::new("material"));
            assert!(!set.inner.is_null());
            core.free_desc_set(set);
        });

        thread1.join().unwrap();
        thread2.join().unwrap();
    }

    unsafe fn test_for_races(vars: testing::TestVars) {
        let device = Arc::clone(&vars.swapchain.device);
        let core_data = Arc::new(CoreData::new(device));

        // Spawn two threads that allocate and deallocate in a loop for
        // a while. Not surefire, but better than nothing.

        const NUM_ITERS: usize = 50;

        let layout = Name::new("scene_globals");

        let core = Arc::clone(&core_data);
        let thread1 = thread::spawn(move || {
            for _ in 0..NUM_ITERS {
                let set0 = core.alloc_desc_set(layout);
                thread::sleep(std::time::Duration::from_micros(50));
                core.free_desc_set(set0);
            }
        });

        let core = Arc::clone(&core_data);
        let thread2 = thread::spawn(move || {
            for _ in 0..NUM_ITERS {
                let set1 = core.alloc_desc_set(layout);
                let set2 = core.alloc_desc_set(layout);
                thread::sleep(std::time::Duration::from_micros(50));
                core.free_desc_set(set1);
                core.free_desc_set(set2);
            }
        });

        thread1.join().unwrap();
        thread2.join().unwrap();
    }

    unit::declare_tests![
        smoke_test,
        test_for_races,
    ];
}

unit::collect_tests![tests];
