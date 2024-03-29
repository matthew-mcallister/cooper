// TODO: Choosing between uniform/storage and dynamic/static should be
// an implementation detail.

use std::borrow::Cow;
use std::ptr;
use std::sync::Arc;

use more_asserts::assert_lt;

use super::*;
use crate::util::{SliceExt, SmallVec};
use crate::{Device, Named, Sampler, StagedCache};

#[derive(Clone, Debug)]
pub struct DescriptorSetLayout {
    device: Arc<Device>,
    inner: vk::DescriptorSetLayout,
    desc: SetLayoutDesc,
    counts: Counts,
    name: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct DescriptorSetLayoutDesc {
    pub bindings: SmallVec<SetLayoutBinding, 4>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DescriptorSetLayoutBinding {
    pub binding: u32,
    pub ty: DescriptorType,
    pub count: u32,
    pub stage_flags: vk::ShaderStageFlags,
    pub samplers: Option<SmallVec<Arc<Sampler>, 2>>,
}

impl Drop for Layout {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            dt.destroy_descriptor_set_layout(self.inner, ptr::null());
        }
    }
}

impl Default for DescriptorSetLayoutBinding {
    fn default() -> Self {
        Self {
            binding: 0,
            ty: Default::default(),
            count: 1,
            stage_flags: vk::ShaderStageFlags::ALL,
            samplers: None,
        }
    }
}

impl_device_derived!(Layout);

fn count_descriptors(bindings: &[SetLayoutBinding]) -> Counts {
    bindings
        .iter()
        .map(|binding| (binding.ty, binding.count))
        .sum()
}

fn validate_layout(desc: &SetLayoutDesc) {
    assert_ne!(desc.bindings.len(), 0);

    for binding in desc.bindings.iter() {
        assert_ne!(binding.count, 0);
    }

    // Ensure that there are no duplicates and there are no
    // redundant permutations of the same bindings in the cache
    for binding in desc.bindings.windows(2) {
        assert_lt!(binding[0].binding, binding[1].binding);
    }

    // TODO: wrap stage flags
}

impl Layout {
    #[inline]
    pub fn new(device: Arc<Device>, desc: SetLayoutDesc) -> Self {
        let dt = device.table();

        validate_layout(&desc);

        let mut samplers: Vec<_> = Vec::new();
        let bindings: Vec<_> = desc
            .bindings
            .iter()
            .map(|binding| {
                if let Some(samplers) = &binding.samplers {
                    assert_eq!(
                        samplers.len(),
                        binding.count as usize,
                        "incorrect sampler count"
                    );
                }
                let vk_samplers: Vec<_> = binding
                    .samplers
                    .iter()
                    .flat_map(|samplers| samplers.iter())
                    .map(|sampler| sampler.inner())
                    .collect();
                let p_immutable_samplers = vk_samplers.c_ptr();
                samplers.push(vk_samplers);
                vk::DescriptorSetLayoutBinding {
                    binding: binding.binding,
                    descriptor_type: binding.ty.into(),
                    descriptor_count: binding.count,
                    stage_flags: binding.stage_flags,
                    p_immutable_samplers,
                }
            })
            .collect();

        let info = vk::DescriptorSetLayoutCreateInfo {
            binding_count: bindings.len() as _,
            p_bindings: bindings.as_ptr(),
            ..Default::default()
        };
        let mut inner = vk::null();
        unsafe {
            dt.create_descriptor_set_layout(&info, ptr::null(), &mut inner)
                .check()
                .unwrap();
        }

        Self {
            device,
            inner,
            counts: count_descriptors(&desc.bindings),
            desc,
            name: None,
        }
    }

    #[inline]
    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    #[inline]
    pub fn inner(&self) -> vk::DescriptorSetLayout {
        self.inner
    }

    #[inline]
    pub fn desc(&self) -> &SetLayoutDesc {
        &self.desc
    }

    #[inline]
    pub fn bindings(&self) -> &[SetLayoutBinding] {
        &self.desc().bindings
    }

    #[inline]
    pub fn counts(&self) -> &Counts {
        &self.counts
    }

    #[inline]
    pub fn required_pool_flags(&self) -> vk::DescriptorPoolCreateFlags {
        Default::default()
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        let name: String = name.into();
        self.name = Some(name.clone());
        unsafe {
            self.device().set_name(self.inner(), name);
        }
    }
}

impl Named for Layout {
    fn name(&self) -> Option<&str> {
        Some(&self.name.as_ref()?)
    }
}

#[derive(Debug)]
pub struct DescriptorSetLayoutCache {
    device: Arc<Device>,
    inner: StagedCache<SetLayoutDesc, Arc<SetLayout>>,
}

impl SetLayoutCache {
    #[inline]
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            device,
            inner: Default::default(),
        }
    }

    #[inline]
    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    #[inline]
    pub fn commit(&mut self) {
        self.inner.commit();
    }

    #[inline]
    pub fn get_committed(&self, desc: &SetLayoutDesc) -> Option<&Arc<SetLayout>> {
        self.inner.get_committed(desc)
    }

    pub fn get_or_create_named(
        &self,
        desc: &SetLayoutDesc,
        name: Option<impl Into<String>>,
    ) -> Cow<Arc<SetLayout>> {
        self.inner.get_or_insert_with(desc, move || {
            let mut layout = SetLayout::new(Arc::clone(self.device()), desc.clone());
            tryopt! { layout.set_name(name?) };
            Arc::new(layout)
        })
    }

    #[inline]
    pub fn get_or_create(&self, desc: &SetLayoutDesc) -> Cow<Arc<SetLayout>> {
        self.get_or_create_named(desc, None: Option<&str>)
    }
}
