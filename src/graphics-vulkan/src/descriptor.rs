use std::ptr;
use std::sync::{Arc, Mutex, RwLock};

use ccore::by_ptr::ByPtr;
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
}

#[derive(Debug)]
pub struct DescriptorSet {
    pub pool: Arc<Pool>,
    pub inner: vk::DescriptorSet,
}

#[derive(Debug)]
pub struct DescriptorPool {
    pub layout: Arc<SetLayout>,
    pub size: u32,
    pub inner: vk::DescriptorPool,
}

impl Drop for Pool {
    fn drop(&mut self) {
        unsafe {
            self.layout.device.table
                .destroy_descriptor_pool(self.inner, ptr::null())
        }
    }
}

/// Allocates descriptors of a single layout.
#[derive(Debug)]
struct Suballocator {
    device: Arc<Device>,
    free: Vec<Set>,
    layout: Arc<SetLayout>,
    sub_pools: Vec<Arc<Pool>>,
}

impl Suballocator {
    fn new(layout: Arc<SetLayout>) -> Self {
        Suballocator {
            device: Arc::clone(&layout.device),
            free: Vec::new(),
            layout,
            sub_pools: Vec::new(),
        }
    }

    fn pool_flags(&self) -> vk::DescriptorPoolCreateFlags {
        let mut flags = Default::default();

        let update_after_bind =
            vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL_BIT_EXT;
        if self.layout.flags.contains(update_after_bind) {
            flags |= vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND_BIT_EXT;
        }

        flags
    }

    unsafe fn grow_by(&mut self, size: u32) {
        // Create a descriptor pool
        let pool_sizes = self.layout.counts.pool_sizes(size);
        let create_info = vk::DescriptorPoolCreateInfo {
            flags: self.pool_flags(),
            max_sets: size,
            pool_size_count: pool_sizes.len() as _,
            p_pool_sizes: pool_sizes.as_ptr(),
            ..Default::default()
        };
        let mut obj = vk::null();
        self.device.table.create_descriptor_pool
            (&create_info as _, ptr::null(), &mut obj as _).check().unwrap();

        let pool = Arc::new(Pool {
            layout: Arc::clone(&self.layout),
            inner: obj,
            size,
        });
        self.sub_pools.push(Arc::clone(&pool));

        // Allocate sets from the pool
        let mut sets = vec![vk::null(); size as usize];
        let layouts = vec![self.layout.inner; size as usize];
        let alloc_info = vk::DescriptorSetAllocateInfo {
            descriptor_pool: pool.inner,
            descriptor_set_count: size,
            p_set_layouts: layouts.as_ptr(),
            ..Default::default()
        };
        self.device.table.allocate_descriptor_sets
            (&alloc_info as _, sets.as_mut_ptr()).check().unwrap();

        let sets = sets.into_iter()
            .map(|inner| Set { pool: Arc::clone(&pool), inner });
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
        assert!(Arc::ptr_eq(&self.layout, &set.pool.layout));
        self.free.push(set);
    }
}

#[derive(Debug)]
pub struct DescriptorAllocator {
    sub_alloc: RwLock<FnvHashMap<ByPtr<Arc<SetLayout>>, Mutex<Suballocator>>>,
}

impl Allocator {
    pub fn new(_device: Arc<Device>) -> Self {
        Allocator {
            sub_alloc: RwLock::new(FnvHashMap::default()),
        }
    }

    pub unsafe fn allocate(&self, layout: &Arc<SetLayout>) -> Set {
        // Allocate from existing sub if present
        let ptr = ByPtr::from_ref(layout);
        #[allow(unreachable_code)]
        let _: Option<_> = try {
            let map = self.sub_alloc.read().unwrap();
            let mut sub = map.get(ptr)?.lock().unwrap();
            return sub.allocate();
        };

        // Create a new sub for this layout.
        // Lucky for us, we can perform the allocation before inserting.
        let mut sub = Suballocator::new(Arc::clone(&layout));
        let set = sub.allocate();

        self.sub_alloc.write().unwrap()
            .insert(ByPtr::clone(ptr), Mutex::new(sub));

        set
    }

    pub fn free(&self, set: Set) {
        let layout = ByPtr::from_ref(&set.pool.layout);
        self.sub_alloc.read().unwrap()[layout].lock().unwrap().free(set);
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
    use super::*;

    unsafe fn smoke_test(swapchain: Arc<Swapchain>) {
        let allocator =
            Arc::new(DescriptorAllocator::new(Arc::clone(&swapchain.device)));

        let device = Arc::clone(&swapchain.device);
        let descriptors = Arc::clone(&allocator);
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
            let layout = Arc::new(SetLayout::new(device, &args));

            let set = descriptors.allocate(&layout);
            let _ = descriptors.allocate(&layout);
            let _ = descriptors.allocate(&layout);

            descriptors.free(set);
        });

        let device = Arc::clone(&swapchain.device);
        let descriptors = Arc::clone(&allocator);
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
            let layout = Arc::new(SetLayout::new(device, &args));

            let set = descriptors.allocate(&layout);
            descriptors.free(set);
        });

        thread1.join().unwrap();
        thread2.join().unwrap();
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
