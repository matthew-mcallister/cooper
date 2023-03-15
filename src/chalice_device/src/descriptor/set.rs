use std::ptr;
use std::sync::{Arc, Weak};

use derivative::Derivative;
use log::trace;
use more_asserts::assert_le;
use parking_lot::Mutex;

use super::*;
use crate::*;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct DescriptorSet {
    #[derivative(Debug(format_with = "write_named::<DescriptorSetLayout>"))]
    pub(crate) layout: Arc<DescriptorSetLayout>,
    pub(crate) pool: Weak<Mutex<DescriptorPool>>,
    pub(crate) inner: vk::DescriptorSet,
    pub(crate) name: Option<String>,
}

impl Drop for Set {
    fn drop(&mut self) {
        if let Some(pool) = Weak::upgrade(&self.pool) {
            unsafe {
                pool.lock().free(self);
            }
        }
    }
}

impl Set {
    #[inline]
    pub fn device(&self) -> &Arc<Device> {
        self.layout.device()
    }

    #[inline]
    pub fn inner(&self) -> vk::DescriptorSet {
        self.inner
    }

    #[inline]
    pub fn layout(&self) -> &Arc<DescriptorSetLayout> {
        &self.layout
    }

    #[inline]
    pub fn write_buffer(&mut self, binding: u32, buffer: BufferRange<'_>) {
        self.write_buffers(binding, 0, std::slice::from_ref(&buffer));
    }

    /// Writes uniform or storage buffers. Doesn't work with texel
    /// buffers as they require a buffer view object.
    // N.B. direct writes scale poorly compared to update templates.
    pub fn write_buffers(&mut self, binding: u32, first_element: u32, buffers: &[BufferRange<'_>]) {
        let dt = &self.layout.device().table;
        assert_ne!(buffers.len(), 0);

        let layout_binding = &self.layout.bindings()[binding as usize];
        let len = buffers.len() as u32;
        let ty = layout_binding.ty;

        // Validation
        {
            // N.B. Overrunning writes are actually allowed by the spec
            assert!(first_element + len <= layout_binding.count);
            for buffer in buffers.iter() {
                let required = match buffer.buffer.binding().unwrap() {
                    BufferBinding::Uniform => DescriptorType::UniformBuffer,
                    BufferBinding::Storage => DescriptorType::StorageBuffer,
                    _ => panic!("incompatible descriptor type"),
                };
                assert_eq!(ty, required);
            }
        }

        let info: Vec<_> = buffers
            .iter()
            .map(|buffer| buffer.descriptor_info())
            .collect();
        let writes = [vk::WriteDescriptorSet {
            dst_set: self.inner(),
            dst_binding: binding,
            dst_array_element: first_element,
            descriptor_count: info.len() as _,
            descriptor_type: ty.into(),
            p_buffer_info: info.as_ptr(),
            ..Default::default()
        }];
        unsafe {
            dt.update_descriptor_sets(writes.len() as _, writes.as_ptr(), 0, ptr::null());
        }
    }

    #[inline]
    pub fn write_image(
        &mut self,
        binding: u32,
        element: u32,
        view: &ImageView,
        layout: vk::ImageLayout,
        sampler: Option<&Sampler>,
    ) {
        let samplers: &[&Sampler] = sampler.as_ref().map_or(&[], |s| std::slice::from_ref(s));
        self.write_image_samplers(binding, element, &[(view, layout)], samplers);
    }

    pub fn write_sampler(&mut self, binding: u32, element: u32, sampler: &Sampler) {
        self.write_image_samplers(binding, element, &[], std::slice::from_ref(&sampler))
    }

    /// Writes images and/or samplers to the descriptor set.
    pub fn write_image_samplers(
        &mut self,
        binding: u32,
        first_element: u32,
        views: &[(&ImageView, vk::ImageLayout)],
        samplers: &[&Sampler],
    ) {
        trace!(
            concat!(
                "DescriptorSet::write_image_samplers(self: {:?}, binding: {}, ",
                "first_element: {}, views: {:?}, samplers: {:?})",
            ),
            fmt_named(&*self),
            binding,
            first_element,
            views,
            samplers,
        );

        if !views.is_empty() && !samplers.is_empty() {
            assert_eq!(samplers.len(), views.len());
        }

        let count = (views.len() | samplers.len()) as u32;
        let layout_binding = &self.layout.bindings()[binding as usize];
        let ty = layout_binding.ty;

        // Validation
        assert_le!(first_element + count, layout_binding.count);
        for i in 0..(count as usize) {
            use vk::ImageLayout as Il;
            use DescriptorType as Dt;

            if let Some(&(ref view, layout)) = views.get(i) {
                let flags = view.image().flags();
                match ty {
                    Dt::CombinedImageSampler | Dt::SampledImage => {
                        assert!(!flags.contains(ImageFlags::NO_SAMPLE))
                    }
                    Dt::StorageImage => assert!(flags.contains(ImageFlags::STORAGE)),
                    Dt::InputAttachment => assert!(flags.contains(ImageFlags::INPUT_ATTACHMENT)),
                    _ => unreachable!(),
                }
                match ty {
                    Dt::CombinedImageSampler | Dt::SampledImage => {
                        assert_eq!(layout, Il::SHADER_READ_ONLY_OPTIMAL)
                    }
                    Dt::StorageImage => assert_eq!(layout, Il::GENERAL),
                    _ => {}
                }
                if ty == Dt::CombinedImageSampler && layout_binding.samplers.is_none() {
                    assert!(samplers.get(i).is_some());
                }
            }
        }

        let mut info: Vec<vk::DescriptorImageInfo> = Vec::new();
        info.resize(count as _, Default::default());
        for i in 0..(count as usize) {
            if let Some(&(ref view, layout)) = views.get(i) {
                info[i].image_view = view.inner();
                info[i].image_layout = layout;
            }
            if let Some(sampler) = samplers.get(i) {
                info[i].sampler = sampler.inner();
            }
        }
        let writes = [vk::WriteDescriptorSet {
            dst_set: self.inner(),
            dst_binding: binding,
            dst_array_element: first_element,
            descriptor_count: info.len() as _,
            descriptor_type: ty.into(),
            p_image_info: info.as_ptr(),
            ..Default::default()
        }];
        unsafe {
            self.device().table.update_descriptor_sets(
                writes.len() as _,
                writes.as_ptr(),
                0,
                ptr::null(),
            );
        }
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        let name: String = name.into();
        self.name = Some(name.clone());
        unsafe {
            self.device().set_name(self.inner(), name);
        }
    }
}

impl Named for Set {
    fn name(&self) -> Option<&str> {
        Some(&self.name.as_ref()?)
    }
}
