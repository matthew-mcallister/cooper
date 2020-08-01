// TODO: Choosing between uniform/storage and dynamic/static should be
// an implementation detail.

use std::borrow::Cow;
use std::ptr;
use std::sync::Arc;

use more_asserts::assert_lt;
use prelude::SliceExt;

use crate::{Device, Named, Sampler, StagedCache};
use crate::util::SmallVec;
use super::*;

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

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
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
        unsafe { dt.destroy_descriptor_set_layout(self.inner, ptr::null()); }
    }
}

impl_device_derived!(Layout);

fn count_descriptors(bindings: &[SetLayoutBinding]) -> Counts {
    bindings.iter().map(|binding| (binding.ty, binding.count)).sum()
}

fn validate_layout(desc: &SetLayoutDesc) {
    assert_ne!(desc.bindings.len(), 0);
    let mut b0 = desc.bindings[0].binding;
    for binding in desc.bindings[1..].iter() {
        assert_ne!(binding.count, 0);
        // Ensure that there are no duplicates and there are no
        // redundant permutations of the same bindings in the cache
        let b1 = binding.binding;
        assert_lt!(b0, b1);
        b0 = b1;
    }
    // TODO: wrap stage flags
}

impl Layout {
    pub fn new(device: Arc<Device>, desc: SetLayoutDesc) -> Self {
        let dt = device.table();

        validate_layout(&desc);

        // TODO: I think this could be made simpler with a staticvec
        let mut samplers: SmallVec<_, 4> = SmallVec::new();
        let bindings: SmallVec<_, 4> = desc.bindings.iter().map(|binding| {
            if let Some(samplers) = &binding.samplers {
                assert_eq!(samplers.len(), binding.count as usize);
            }
            let vk_samplers: SmallVec<_, 2> = binding.samplers.iter()
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
        }).collect();

        let info = vk::DescriptorSetLayoutCreateInfo {
            binding_count: bindings.len() as _,
            p_bindings: bindings.as_ptr(),
            ..Default::default()
        };
        let mut inner = vk::null();
        unsafe {
            dt.create_descriptor_set_layout(&info, ptr::null(), &mut inner)
                .check().unwrap();
        }

        Self {
            device,
            inner,
            counts: count_descriptors(&desc.bindings),
            desc,
            name: None,
        }
    }

    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    pub fn inner(&self) -> vk::DescriptorSetLayout {
        self.inner
    }

    pub fn desc(&self) -> &SetLayoutDesc {
        &self.desc
    }

    pub fn bindings(&self) -> &[SetLayoutBinding] {
        &self.desc().bindings
    }

    pub fn counts(&self) -> &Counts {
        &self.counts
    }

    pub fn required_pool_flags(&self) -> vk::DescriptorPoolCreateFlags {
        Default::default()
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

#[derive(Debug)]
pub struct DescriptorSetLayoutCache {
    device: Arc<Device>,
    inner: StagedCache<SetLayoutDesc, Arc<SetLayout>>,
}

impl SetLayoutCache {
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            device,
            inner: Default::default(),
        }
    }

    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    pub fn commit(&mut self) {
        self.inner.commit();
    }

    pub fn get_committed(&self, desc: &SetLayoutDesc) ->
        Option<&Arc<SetLayout>>
    {
        self.inner.get_committed(desc)
    }

    pub fn get_or_create_named(
        &self,
        desc: &SetLayoutDesc,
        name: Option<String>,
    ) -> Cow<Arc<SetLayout>> {
        self.inner.get_or_insert_with(desc, move || {
            let mut layout =
                SetLayout::new(Arc::clone(self.device()), desc.clone());
            tryopt! { layout.set_name(name?) };
            Arc::new(layout)
        })
    }

    pub fn get_or_create(&self, desc: &SetLayoutDesc) -> Cow<Arc<SetLayout>> {
        self.get_or_create_named(desc, None)
    }
}
