// TODO: Choosing between uniform/storage and dynamic/static should be
// an implementation detail.

use std::ptr;
use std::sync::Arc;

use crate::*;
use super::*;

#[derive(Clone, Debug)]
pub struct DescriptorSetLayout {
    device: Arc<Device>,
    inner: vk::DescriptorSetLayout,
    flags: vk::DescriptorSetLayoutCreateFlags,
    bindings: Box<[vk::DescriptorSetLayoutBinding]>,
    counts: Counts,
    name: Option<String>,
}

impl Drop for Layout {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe { dt.destroy_descriptor_set_layout(self.inner, ptr::null()); }
    }
}

fn count_descriptors(bindings: &[vk::DescriptorSetLayoutBinding]) -> Counts {
    bindings.iter()
        .map(|binding| (binding.descriptor_type, binding.descriptor_count))
        .sum()
}

impl Layout {
    pub unsafe fn new(
        device: Arc<Device>,
        flags: vk::DescriptorSetLayoutCreateFlags,
        bindings: &[vk::DescriptorSetLayoutBinding],
    ) -> Self {
        let dt = &*device.table;

        // Validation
        {
            for binding in bindings.iter() {
                assert!(is_valid_type(binding.descriptor_type));
            }
        }

        let create_info = vk::DescriptorSetLayoutCreateInfo {
            flags,
            binding_count: bindings.len() as _,
            p_bindings: bindings.as_ptr(),
            ..Default::default()
        };
        let counts = count_descriptors(bindings);
        let mut inner = vk::null();
        dt.create_descriptor_set_layout(&create_info, ptr::null(), &mut inner)
            .check().unwrap();
        Self {
            device,
            inner,
            flags,
            bindings: bindings.into(),
            counts,
            name: None,
        }
    }

    pub unsafe fn from_bindings(
        device: Arc<Device>,
        bindings: &[vk::DescriptorSetLayoutBinding],
    ) -> Self {
        Self::new(device, Default::default(), bindings)
    }

    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    pub fn inner(&self) -> vk::DescriptorSetLayout {
        self.inner
    }

    pub fn flags(&self) -> vk::DescriptorSetLayoutCreateFlags {
        self.flags
    }

    pub fn bindings(&self) -> &[vk::DescriptorSetLayoutBinding] {
        &self.bindings
    }

    pub fn counts(&self) -> &Counts {
        &self.counts
    }

    pub fn required_pool_flags(&self) -> vk::DescriptorPoolCreateFlags {
        let mut flags = Default::default();

        let update_after_bind =
            vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL_BIT_EXT;
        if self.flags.contains(update_after_bind) {
            flags |= vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND_BIT_EXT;
        }

        flags
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        let name: String = name.into();
        self.name = Some(name.clone());
        unsafe { self.device().set_name(self.inner(), name); }
    }
}

impl Named for Layout {
    fn name(&self) -> Option<&str> {
        Some(&self.name.as_ref()?)
    }
}
