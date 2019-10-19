use std::ptr;
use std::sync::Arc;

use fnv::FnvHashMap;
use parking_lot::{Mutex, RwLock};

use crate::*;

// Sorry if this causes more confusion than good
use self::{
    DescriptorCounts as Counts, DescriptorSet as Set,
    DescriptorSetLayout as Layout, DescriptorManager as Manager,
};

/// Maps descriptor type to descriptor count for a set layout.
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
            *inner.entry(ty).or_insert(0) += count;
        }
        Counts { inner }
    }
}

/// Tells the allocator how to allocate sets of a particular layout.
#[derive(Clone, Copy, Debug)]
pub struct DescriptorSetAllocPolicy {
    pub pool_size: u32,
}

impl Default for DescriptorSetAllocPolicy {
    fn default() -> Self {
        DescriptorSetAllocPolicy {
            pool_size: 4,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DescriptorSetLayout {
    inner: vk::DescriptorSetLayout,
    flags: vk::DescriptorSetLayoutCreateFlags,
    counts: Counts,
    alloc_policy: DescriptorSetAllocPolicy,
}

#[derive(Debug, Default)]
pub struct DescriptorSetLayoutCreateArgs<'a> {
    pub flags: vk::DescriptorSetLayoutCreateFlags,
    pub bindings: &'a [vk::DescriptorSetLayoutBinding],
    pub binding_flags: Option<&'a [vk::DescriptorBindingFlagsEXT]>,
    pub alloc_policy: DescriptorSetAllocPolicy,
}

impl DescriptorSetLayout {
    unsafe fn new(
        device: &Device,
        params: &DescriptorSetLayoutCreateArgs,
    ) -> Self {
        let dt = &*device.table;

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
        dt.create_descriptor_set_layout(&create_info, ptr::null(), &mut inner)
            .check().unwrap();

        Layout {
            inner,
            flags: params.flags,
            counts: DescriptorCounts::from_bindings(params.bindings),
            alloc_policy: params.alloc_policy,
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

    pub unsafe fn create_pool(&self, device: &Device, size: u32) ->
        vk::DescriptorPool
    {
        let dt = &device.table;
        let layout = &self;

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
}

#[derive(Debug)]
pub struct DescriptorSet {
    inner: vk::DescriptorSet,
    layout: String,
    pool: usize,
}

impl DescriptorSet {
    pub fn inner(&self) -> vk::DescriptorSet {
        self.inner
    }

    pub fn layout(&self) -> &str {
        &self.layout
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
#[derive(Debug)]
struct Suballocator {
    device: Arc<Device>,
    free: Vec<Set>,
    sub_pools: Vec<Subpool>,
    // This pointer eases control flow burden. It couples the allocator
    // to set layout storage, so both are behind a unified interface,
    // the DescriptorManager.
    layout: *const Layout,
    layout_name: String,
}

impl Drop for Suballocator {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            for pool in self.sub_pools.iter() {
                dt.destroy_descriptor_pool(pool.inner, ptr::null());
            }
        }
    }
}

impl Suballocator {
    unsafe fn grow_by(&mut self, size: u32) {
        assert!(size > 0);
        let layout = &*self.layout; // raw dereference
        let layout_name = &self.layout_name;

        // Add new descriptor pool
        let pool_inner = layout.create_pool(&self.device, size);
        let pool = Subpool { inner: pool_inner, size };

        // Allocate all possible sets
        let sets = pool.alloc_sets(&self.device, layout.inner);

        // Add to free list
        let pool_idx = self.sub_pools.len();
        let sets = sets.into_iter()
            .map(|inner| Set {
                inner,
                layout: layout_name.clone(),
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
        assert_eq!(set.layout, self.layout_name);
        self.free.push(set);
    }
}

#[derive(Debug)]
pub struct DescriptorManager {
    device: Arc<Device>,
    layouts: FnvHashMap<String, Box<Layout>>,
    // TODO: RwLock + HashMap + Mutex is seemingly inferior to a true
    // concurrent hash map. Worth switching to?
    sub_alloc: RwLock<FnvHashMap<String, Mutex<Suballocator>>>,
}

unsafe impl Send for DescriptorManager {}
unsafe impl Sync for DescriptorManager {}

impl Drop for Manager {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            for layout in self.layouts.values() {
                dt.destroy_descriptor_set_layout(layout.inner, ptr::null());
            }
        }
    }
}

impl Manager {
    pub fn new(device: Arc<Device>) -> Self {
        Manager {
            device,
            layouts: Default::default(),
            sub_alloc: RwLock::new(Default::default()),
        }
    }

    pub unsafe fn create_layout(
        &mut self,
        name: String,
        params: &DescriptorSetLayoutCreateArgs,
    ) {
        let layout = Layout::new(&self.device, params);
        self.layouts.insert(name, Box::new(layout));
    }

    pub fn get_layout(&self, layout: impl AsRef<str>) -> &Layout {
        &self.layouts[layout.as_ref()]
    }

    pub unsafe fn allocate(&self, layout: impl AsRef<str>) -> Set {
        let layout_name = layout.as_ref();

        // Allocate from existing sub if present
        #[allow(unreachable_code)]
        let _: Option<_> = try {
            let map = self.sub_alloc.read();
            let mut sub = map.get(layout_name)?.lock();
            return sub.allocate();
        };

        // Acquire lock and re-check condition to avoid racing to insert
        // the new sub.
        let mut sub_alloc = self.sub_alloc.write();
        if let Some(sub) = sub_alloc.get(layout_name) {
            return sub.lock().allocate();
        }

        // Create a new sub for this layout.
        let layout = &*self.layouts[layout_name];
        let mut sub = Suballocator {
            device: Arc::clone(&self.device),
            free: Vec::new(),
            sub_pools: Vec::new(),
            layout,
            layout_name: layout_name.to_owned(),
        };
        // Lucky for us, we can perform the allocation before inserting.
        let set = sub.allocate();

        sub_alloc.insert(layout_name.to_owned(), Mutex::new(sub));

        set
    }

    pub fn free(&self, set: Set) {
        use std::ops::Index;
        self.sub_alloc.read()
            .index(&set.layout)
            .lock()
            .free(set.into());
    }
}

pub unsafe fn create_set_layouts(descs: &mut Manager) {
    let bindings = [vk::DescriptorSetLayoutBinding {
        binding: 0,
        descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
        descriptor_count: 1,
        stage_flags: vk::ShaderStageFlags::VERTEX_BIT
            | vk::ShaderStageFlags::FRAGMENT_BIT,
        ..Default::default()
    }];
    let args = DescriptorSetLayoutCreateArgs {
        bindings: &bindings[..],
        ..Default::default()
    };
    descs.create_layout("scene_globals".into(), &args);

    let bindings = [vk::DescriptorSetLayoutBinding {
        binding: 0,
        descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        descriptor_count: 1,
        stage_flags: vk::ShaderStageFlags::FRAGMENT_BIT,
        ..Default::default()
    }];
    let args = DescriptorSetLayoutCreateArgs {
        bindings: &bindings[..],
        ..Default::default()
    };
    descs.create_layout("material".into(), &args);
}

#[cfg(test)]
mod tests {
    use std::thread;
    use vk::traits::*;
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let device = Arc::clone(&vars.swapchain.device);

        let mut allocator = DescriptorManager::new(Arc::clone(&device));
        create_set_layouts(&mut allocator);
        let allocator = Arc::new(allocator);

        let descriptors = Arc::clone(&allocator);
        let thread1 = thread::spawn(move || {
            let layout = "scene_globals";
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

        let descriptors = Arc::clone(&allocator);
        let thread2 = thread::spawn(move || {
            let set = descriptors.allocate("material");
            assert!(!set.inner.is_null());
            descriptors.free(set);
        });

        thread1.join().unwrap();
        thread2.join().unwrap();
    }

    unsafe fn test_for_races(vars: testing::TestVars) {
        let device = Arc::clone(&vars.swapchain.device);

        let mut allocator = DescriptorManager::new(Arc::clone(&device));
        create_set_layouts(&mut allocator);
        let allocator = Arc::new(allocator);

        // Spawn two threads that allocate and deallocate in a loop for
        // a while. Not surefire, but better than nothing.

        const NUM_ITERS: usize = 50;

        let layout = "scene_globals";

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
