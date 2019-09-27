use std::ptr;
use std::sync::{Arc, Mutex, RwLock};

use ccore::node::{Id, NodeArray};
use fnv::FnvHashMap;

use crate::*;

// Sorry if this causes more confusion than good
use self::{
    DescriptorAllocator as Allocator, DescriptorCounts as Counts,
    DescriptorPool as Pool, DescriptorSet as Set,
    DescriptorSetLayout as SetLayout,
};

macro_rules! insert_unique {
    ($map:expr; $key:expr => $val:expr) => {
        assert!($map.insert($key, $val).is_none());
    }
}

#[derive(Clone, Debug)]
pub struct DescriptorCounts {
    pub inner: FnvHashMap<vk::DescriptorType, u32>,
}

impl Counts {
    pub fn pool_sizes(&self, multiplier: u32) -> Vec<vk::DescriptorPoolSize> {
        self.inner.iter()
            .map(|(&ty, &count)| vk::DescriptorPoolSize {
                ty,
                descriptor_count: count * multiplier,
            })
            .collect()
    }

    pub fn from_bindings(bindings: &[vk::DescriptorSetLayoutBinding]) -> Self {
        let mut inner = FnvHashMap::default();
        for binding in bindings.iter() {
            let ty = binding.descriptor_type;
            let count = binding.descriptor_count;
            insert_unique!(inner; ty => count);
        }
        Counts { inner }
    }
}

#[derive(Clone, Debug)]
pub struct DescriptorSetLayout {
    pub device: Arc<Device>,
    pub inner: vk::DescriptorSetLayout,
    pub flags: vk::DescriptorSetLayoutCreateFlags,
    pub counts: Counts,
}

impl Drop for SetLayout {
    fn drop(&mut self) {
        unsafe {
            self.device.table
                .destroy_descriptor_set_layout(self.inner, ptr::null());
        }
    }
}

#[derive(Debug, Default)]
#[non_exhaustive]
pub struct DescriptorSetLayoutCreateArgs<'a> {
    flags: vk::DescriptorSetLayoutCreateFlags,
    bindings: &'a [vk::DescriptorSetLayoutBinding],
    binding_flags: Option<&'a [vk::DescriptorBindingFlagsEXT]>,
}

impl SetLayout {
    pub unsafe fn new(
        device: Arc<Device>,
        params: &DescriptorSetLayoutCreateArgs,
    ) -> Self {
        let (p_next, _flag_create_info);
        if let Some(binding_flags) = params.binding_flags {
            assert_eq!(binding_flags.len(), params.bindings.len());
             _flag_create_info =
                vk::DescriptorSetLayoutBindingFlagsCreateInfoEXT {
                    binding_count: binding_flags.len() as _,
                    p_binding_flags: binding_flags.as_ptr(),
                    ..Default::default()
                };
            p_next = &_flag_create_info as *const _ as _;
        } else {
            _flag_create_info = Default::default();
            p_next = ptr::null();
        }

        let create_info = vk::DescriptorSetLayoutCreateInfo {
            p_next,
            flags: params.flags,
            binding_count: params.bindings.len() as _,
            p_bindings: params.bindings.as_ptr(),
            ..Default::default()
        };
        let mut inner = vk::null();
        device.table.create_descriptor_set_layout
            (&create_info as _, ptr::null(), &mut inner as _).check().unwrap();

        SetLayout {
            device,
            inner,
            flags: params.flags,
            counts: DescriptorCounts::from_bindings(params.bindings),
        }
    }

    pub fn pool_flags(&self) -> vk::DescriptorPoolCreateFlags {
        let mut flags = Default::default();

        let update_after_bind =
            vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL_BIT_EXT;
        if self.flags.contains(update_after_bind) {
            flags |= vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND_BIT_EXT;
        }

        flags
    }
}

#[derive(Debug)]
pub struct DescriptorSet {
    pub layout: Id<SetLayout>,
    pub pool: usize,
    pub inner: vk::DescriptorSet,
}

#[derive(Debug)]
pub struct DescriptorPool {
    pub device: Arc<Device>,
    pub layout: Id<SetLayout>,
    pub size: u32,
    pub inner: vk::DescriptorPool,
}

impl Drop for Pool {
    fn drop(&mut self) {
        unsafe {
            self.device.table
                .destroy_descriptor_pool(self.inner, ptr::null())
        }
    }
}

/// Allocates descriptors of a single layout.
#[derive(Debug)]
struct Suballocator {
    device: Arc<Device>,
    // TODO: deadlock hazard---eliminate this lock
    layout_table: Arc<RwLock<NodeArray<SetLayout>>>,
    free: Vec<Set>,
    layout: Id<SetLayout>,
    sub_pools: Vec<Pool>,
}

impl Suballocator {
    fn new(
        device: Arc<Device>,
        layout_table: Arc<RwLock<NodeArray<SetLayout>>>,
        layout: Id<SetLayout>,
    ) -> Self {
        Suballocator {
            device,
            layout_table,
            free: Vec::new(),
            layout,
            sub_pools: Vec::new(),
        }
    }
}

impl Suballocator {
    unsafe fn grow_by(&mut self, size: u32) {
        let layout_id = self.layout;

        // Create a descriptor pool
        let (layout, pool_sizes, flags);
        {
            let layout_obj = &self.layout_table.read().unwrap()[layout_id];
            layout = layout_obj.inner;
            pool_sizes = layout_obj.counts.pool_sizes(size);
            flags = layout_obj.pool_flags();
        };

        let create_info = vk::DescriptorPoolCreateInfo {
            flags,
            max_sets: size,
            pool_size_count: pool_sizes.len() as _,
            p_pool_sizes: pool_sizes.as_ptr(),
            ..Default::default()
        };
        let mut obj = vk::null();
        self.device.table.create_descriptor_pool
            (&create_info as _, ptr::null(), &mut obj as _).check().unwrap();

        let pool = Pool {
            device: Arc::clone(&self.device),
            layout: layout_id,
            inner: obj,
            size,
        };
        let pool_idx = self.sub_pools.len();
        self.sub_pools.push(pool);

        // Allocate sets from the pool
        let mut sets = vec![vk::null(); size as usize];
        let layouts = vec![layout; size as usize];
        let alloc_info = vk::DescriptorSetAllocateInfo {
            descriptor_pool: obj,
            descriptor_set_count: size,
            p_set_layouts: layouts.as_ptr(),
            ..Default::default()
        };
        self.device.table.allocate_descriptor_sets
            (&alloc_info as _, sets.as_mut_ptr()).check().unwrap();

        let sets = sets.into_iter()
            .map(|inner| Set { layout: layout_id, pool: pool_idx, inner });
        self.free.extend(sets);
    }

    unsafe fn grow(&mut self) {
        let size = self.sub_pools.pop()
            .map_or(4, |pool| (3 * pool.size + 1) / 2);
        self.grow_by(size);
    }

    unsafe fn allocate(&mut self) -> Set {
        if self.free.is_empty() { self.grow(); }
        self.free.pop().unwrap()
    }

    fn free(&mut self, set: Set) {
        assert_eq!(self.layout, set.layout);
        self.free.push(set);
    }
}

#[derive(Debug)]
pub struct DescriptorAllocator {
    device: Arc<Device>,
    // TODO: RwLock + HashMap + Mutex is technically inferior to a true
    // concurrent hash map. Should we switch?
    sub_alloc: RwLock<FnvHashMap<Id<SetLayout>, Mutex<Suballocator>>>,
    layout_table: Arc<RwLock<NodeArray<SetLayout>>>,
}

impl Allocator {
    pub fn new(
        device: Arc<Device>,
        layout_table: Arc<RwLock<NodeArray<SetLayout>>>,
    ) -> Self {
        Allocator {
            device,
            sub_alloc: RwLock::new(FnvHashMap::default()),
            layout_table,
        }
    }

    pub unsafe fn allocate(&self, layout: Id<SetLayout>) -> Set {
        // Allocate from existing sub if present
        #[allow(unreachable_code)]
        let _: Option<_> = try {
            let map = self.sub_alloc.read().unwrap();
            let mut sub = map.get(&layout)?.lock().unwrap();
            return sub.allocate();
        };

        // Acquire lock and re-check condition to avoid racing to insert
        // the new sub.
        let mut sub_alloc = self.sub_alloc.write().unwrap();
        if let Some(sub) = sub_alloc.get(&layout) {
            return sub.lock().unwrap().allocate();
        }

        // Create a new sub for this layout.
        let mut sub = Suballocator::new(
            Arc::clone(&self.device),
            Arc::clone(&self.layout_table),
            layout,
        );
        // Lucky for us, we can perform the allocation before inserting.
        let set = sub.allocate();

        sub_alloc.insert(layout, Mutex::new(sub));

        set
    }

    pub fn free(&self, set: Set) {
        self.sub_alloc.read().unwrap()[&set.layout].lock().unwrap().free(set);
    }
}

// TODO: We might want an abstraction over descriptor set bindings.
//
// Desired features:
// - Easily compose new descriptor set layouts
// - Trade multiple set layouts for a single combined layout
// - No longer hardcode which slots descriptors are bound to

#[cfg(test)]
mod tests {
    use std::thread;
    use ccore::node::NodeArray;
    use vk::traits::*;
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let swapchain = vars.swapchain;
        let device = Arc::clone(&swapchain.device);
        let set_layouts = Arc::new(RwLock::new(NodeArray::new()));

        let layouts = Arc::clone(&set_layouts);
        let allocator = Arc::new(DescriptorAllocator::new(device, layouts));

        let device = Arc::clone(&swapchain.device);
        let descriptors = Arc::clone(&allocator);
        let layouts = Arc::clone(&set_layouts);
        let thread1 = thread::spawn(move || {
            let bindings = [vk::DescriptorSetLayoutBinding {
                binding: 0,
                descriptor_type: vk::DescriptorType::SAMPLED_IMAGE,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::FRAGMENT_BIT,
                ..Default::default()
            }];
            let args = DescriptorSetLayoutCreateArgs {
                bindings: &bindings[..],
                ..Default::default()
            };
            let layout = SetLayout::new(device, &args);
            let layout = layouts.write().unwrap().add(layout);

            let set0 = descriptors.allocate(layout);
            let sets = [
                descriptors.allocate(layout),
                descriptors.allocate(layout),
                descriptors.allocate(layout),
            ];
            descriptors.free(set0);

            assert!(!sets.iter().any(|set| set.inner.is_null()));
            assert_ne!(sets[0].inner, sets[1].inner);
            assert_ne!(sets[1].inner, sets[2].inner);
            assert_ne!(sets[2].inner, sets[0].inner);
        });

        let device = Arc::clone(&swapchain.device);
        let descriptors = Arc::clone(&allocator);
        let layouts = Arc::clone(&set_layouts);
        let thread2 = thread::spawn(move || {
            let bindings = [vk::DescriptorSetLayoutBinding {
                binding: 0,
                descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::COMPUTE_BIT,
                ..Default::default()
            }];
            let args = DescriptorSetLayoutCreateArgs {
                bindings: &bindings[..],
                ..Default::default()
            };
            let layout = SetLayout::new(device, &args);
            let layout = layouts.write().unwrap().add(layout);

            let set = descriptors.allocate(layout);
            assert!(!set.inner.is_null());
            descriptors.free(set);
        });

        thread1.join().unwrap();
        thread2.join().unwrap();
    }

    unsafe fn test_for_races(vars: testing::TestVars) {
        let swapchain = vars.swapchain;
        let device = Arc::clone(&swapchain.device);

        let layouts = Arc::new(RwLock::new(NodeArray::new()));

        let bindings = [vk::DescriptorSetLayoutBinding {
            binding: 0,
            descriptor_type: vk::DescriptorType::SAMPLED_IMAGE,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::FRAGMENT_BIT,
            ..Default::default()
        }];
        let args = DescriptorSetLayoutCreateArgs {
            bindings: &bindings[..],
            ..Default::default()
        };
        let layout = SetLayout::new(Arc::clone(&device), &args);
        let layout = layouts.write().unwrap().add(layout);

        let allocator = Arc::new(DescriptorAllocator::new(device, layouts));

        // Spawn two threads that allocate and deallocate in a loop for
        // a while. Not surefire, but better than nothing.

        const NUM_ITERS: usize = 50;

        let descriptors = Arc::clone(&allocator);
        let thread1 = thread::spawn(move || {
            for _ in 0..NUM_ITERS {
                let set0 = descriptors.allocate(layout);
                thread::sleep(std::time::Duration::from_micros(50));
                descriptors.free(set0);
            }
        });

        let descriptors = Arc::clone(&allocator);
        let thread2 = thread::spawn(move || {
            for _ in 0..NUM_ITERS {
                let set1 = descriptors.allocate(layout);
                let set2 = descriptors.allocate(layout);
                thread::sleep(std::time::Duration::from_micros(50));
                descriptors.free(set1);
                descriptors.free(set2);
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
