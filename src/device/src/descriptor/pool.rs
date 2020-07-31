use std::ptr;
use std::sync::{Arc, Weak};

use enum_map::{EnumMap, enum_map};
use log::trace;
use parking_lot::Mutex;

use crate::*;
use super::*;

/// Fixed-size general-purpose descriptor pool.
#[derive(Debug)]
pub struct DescriptorPool {
    device: Arc<Device>,
    inner: vk::DescriptorPool,
    flags: vk::DescriptorPoolCreateFlags,
    lifetime: Lifetime,
    // Note: limits are not hard but are informative
    max_sets: u32,
    used_sets: u32,
    max_descriptors: Counts,
    used_descriptors: Counts,
    name: Option<String>,
}

#[derive(Debug)]
pub struct DescriptorHeap {
    pools: EnumMap<Lifetime, Arc<Mutex<DescriptorPool>>>,
}

fn pool_sizes(counts: &DescriptorCounts) -> Vec<vk::DescriptorPoolSize> {
    counts.iter()
        .filter_map(|(ty, n)| (n > 0).then_some(
            vk::DescriptorPoolSize { ty, descriptor_count: n }
        ))
        .collect()
}

impl Drop for Pool {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe { dt.destroy_descriptor_pool(self.inner, ptr::null()); }
    }
}

impl Pool {
    pub fn new(
        device: Arc<Device>,
        max_sets: u32,
        descriptor_counts: Counts,
        lifetime: Lifetime,
    ) -> Self {
        let dt = &*device.table;

        let flags = if lifetime == Lifetime::Static {
            vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET_BIT
        } else { Default::default() };
        let pool_sizes = pool_sizes(&descriptor_counts);
        let create_info = vk::DescriptorPoolCreateInfo {
            flags,
            max_sets,
            p_pool_sizes: pool_sizes.as_ptr(),
            pool_size_count: pool_sizes.len() as _,
            ..Default::default()
        };
        let mut inner = vk::null();
        unsafe {
            dt.create_descriptor_pool(&create_info, ptr::null(), &mut inner)
                .check().unwrap();
        }

        DescriptorPool {
            device,
            inner,
            flags,
            lifetime,
            max_sets,
            used_sets: 0,
            max_descriptors: descriptor_counts,
            used_descriptors: Default::default(),
            name: None,
        }
    }

    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    pub fn inner(&self) -> vk::DescriptorPool {
        self.inner
    }

    pub fn max_sets(&self) -> u32 {
        self.max_sets
    }

    pub fn used_sets(&self) -> u32 {
        self.used_sets
    }

    pub fn max_descriptors(&self) -> &Counts {
        &self.max_descriptors
    }

    pub fn used_descriptors(&self) -> &Counts {
        &self.used_descriptors
    }

    pub fn alloc_many(
        &mut self,
        layout: &Arc<DescriptorSetLayout>,
        count: u32,
    ) -> Vec<DescriptorSet> {
        trace!(
            concat!(
                "DescriptorPool::alloc_many(lifetime: {:?}, layout: {:?}, ",
                "count: {})",
            ),
            self.lifetime, fmt_named(&**layout), count,
        );

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
        unsafe {
            dt.allocate_descriptor_sets(&alloc_info, sets.as_mut_ptr())
                .check().unwrap();
        }

        sets.into_iter().map(|inner| {
            DescriptorSet {
                layout: layout.clone(),
                pool: Weak::new(),
                inner,
                name: None,
            }
        }).collect()
    }

    pub fn alloc(&mut self, layout: &Arc<DescriptorSetLayout>) -> Set {
        self.alloc_many(layout, 1).pop().unwrap()
    }

    pub(super) unsafe fn free(&mut self, set: &Set) {
        // TODO: Maybe assert this is the correct VkDescriptorPool
        trace!("DescriptorPool::free(lifetime: {:?}, set: {:?})",
            self.lifetime, set);

        self.used_sets -= 1;
        self.used_descriptors -= set.layout().counts();

        let dt = &*self.device.table;
        let inner = set.inner();
        let sets = std::slice::from_ref(&inner);
        dt.free_descriptor_sets(self.inner(), sets.len() as _, sets.as_ptr())
            .check().unwrap();
    }

    pub unsafe fn reset(&mut self) {
        let dt = &*self.device.table;
        dt.reset_descriptor_pool(self.inner(), Default::default());
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        let name: String = name.into();
        self.name = Some(name.clone());
        unsafe { self.device().set_name(self.inner(), name); }
    }
}

impl Named for Pool {
    fn name(&self) -> Option<&str> {
        Some(&self.name.as_ref()?)
    }
}

impl Heap {
    pub fn new(device: &Arc<Device>) -> Self {
        let pools = enum_map! {
            Lifetime::Static => {
                let (sets, sizes) = static_descriptor_counts();
                let mut pool = Pool::new(
                    Arc::clone(&device), sets, sizes, Lifetime::Static);
                pool.set_name("static_pool");
                Arc::new(Mutex::new(pool))
            },
            Lifetime::Frame => {
                let (sets, sizes) = frame_descriptor_counts();
                let mut pool = Pool::new(
                    Arc::clone(&device), sets, sizes, Lifetime::Frame);
                pool.set_name("frame_pool");
                Arc::new(Mutex::new(pool))
            },
        };
        Self { pools }
    }

    pub fn alloc_many(
        self: &Arc<Self>,
        lifetime: Lifetime,
        layout: &Arc<DescriptorSetLayout>,
        count: u32,
    ) -> Vec<DescriptorSet> {
        let mut sets = self.pools[lifetime].lock().alloc_many(layout, count);
        if lifetime == Lifetime::Static {
            for set in sets.iter_mut() {
                set.pool = Arc::downgrade(&self.pools[lifetime]);
            }
        }
        sets
    }

    pub fn alloc(
        self: &Arc<Self>,
        lifetime: Lifetime,
        layout: &Arc<DescriptorSetLayout>,
    ) -> Set {
        self.alloc_many(lifetime, layout, 1).pop().unwrap()
    }

    pub unsafe fn clear_frame(&self) {
        self.pools[Lifetime::Frame].lock().reset();
    }
}

/// Returns a reasonable number of descriptor sets and pool sizes for
/// a global descriptor pool.
fn global_descriptor_counts(max_sets: u32) -> (u32, Counts) {
    #![allow(clippy::identity_op)]
    let max_descs = [
        (vk::DescriptorType::SAMPLER,                   1 * max_sets),
        (vk::DescriptorType::COMBINED_IMAGE_SAMPLER,    8 * max_sets),
        (vk::DescriptorType::SAMPLED_IMAGE,             8 * max_sets),
        (vk::DescriptorType::STORAGE_IMAGE,             1 * max_sets),
        (vk::DescriptorType::UNIFORM_TEXEL_BUFFER,      1 * max_sets),
        (vk::DescriptorType::STORAGE_TEXEL_BUFFER,      1 * max_sets),
        (vk::DescriptorType::UNIFORM_BUFFER,            2 * max_sets),
        (vk::DescriptorType::STORAGE_BUFFER,            2 * max_sets),
        (vk::DescriptorType::INPUT_ATTACHMENT,          256),
    ].iter().cloned().collect();
    (max_sets, max_descs)
}

fn static_descriptor_counts() -> (u32, Counts) {
    global_descriptor_counts(0x1_0000)
}

pub(super) fn frame_descriptor_counts() -> (u32, Counts) {
    global_descriptor_counts(0x1000)
}
