use std::ptr;
use std::sync::{Arc, Weak};

use derivative::Derivative;
use log::trace;
use more_asserts::assert_lt;
use parking_lot::Mutex;
use prelude::tryopt;
use vk::traits::*;

use crate::*;
use super::*;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct DescriptorSet {
    #[derivative(Debug(format_with = "write_named::<DescriptorSetLayout>"))]
    crate layout: Arc<DescriptorSetLayout>,
    crate pool: Weak<Mutex<DescriptorPool>>,
    crate inner: vk::DescriptorSet,
    crate name: Option<String>,
}

impl Drop for Set {
    fn drop(&mut self) {
        if let Some(pool) = Weak::upgrade(&self.pool) {
            unsafe { pool.lock().free(self); }
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
    pub fn write_buffer(
        &mut self,
        binding: u32,
        buffer: BufferRange<'_>,
    ) {
        self.write_buffers(binding, 0, std::slice::from_ref(&buffer));
    }

    /// Writes uniform or storage buffers. Doesn't work with texel
    /// buffers as they require a buffer view object.
    // N.B. direct writes scale poorly compared to update templates.
    pub fn write_buffers(
        &mut self,
        binding: u32,
        first_element: u32,
        buffers: &[BufferRange<'_>],
    ) {
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

        let info: Vec<_> = buffers.iter()
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
            dt.update_descriptor_sets
                (writes.len() as _, writes.as_ptr(), 0, ptr::null());
        }
    }

    #[inline]
    pub unsafe fn write_image(
        &mut self,
        binding: u32,
        view: &Arc<ImageView>,
        layout: vk::ImageLayout,
        sampler: Option<&Arc<Sampler>>,
    ) {
        let sampler = tryopt!([sampler?]);
        let samplers = tryopt!(&sampler.as_ref()?[..]);
        self.write_images(binding, 0, &[view], layout, samplers);
    }

    /// Writes images to the descriptor set. Combined image/samplers
    /// must specify an array of samplers.
    // TODO: Perhaps should take an iterator
    pub unsafe fn write_images(
        &mut self,
        binding: u32,
        first_element: u32,
        views: &[&Arc<ImageView>],
        layout: vk::ImageLayout,
        samplers: Option<&[&Arc<Sampler>]>,
    ) {
        if let Some(samplers) = samplers {
            assert_eq!(samplers.len(), views.len());
        }

        for (i, &view) in views.iter().enumerate() {
            let sampler = tryopt!(samplers?[i]);
            let elem = first_element + i as u32;
            self.write_image_element(binding, elem, view, layout, sampler);
        }
    }

    unsafe fn write_image_element(
        &mut self,
        binding: u32,
        element: u32,
        view: &Arc<ImageView>,
        layout: vk::ImageLayout,
        sampler: Option<&Arc<Sampler>>,
    ) {
        trace!(
            concat!(
                "DescriptorSet::write_image_element(self: {:?}, binding: {}, ",
                "element: {}, view: {:?}, layout: {:?}, sampler: {:?})",
            ),
            fmt_named(&*self), binding, element, view, layout, sampler,
        );

        use DescriptorType as Dt;
        use vk::ImageLayout as Il;

        let dt = &self.layout.device().table;
        let layout_binding = &self.layout.bindings()[binding as usize];
        let ty = layout_binding.ty;

        let sampler = tryopt!(sampler?.inner()).unwrap_or(vk::null());

        // Validation
        {
            assert_lt!(element, layout_binding.count);
            let flags = view.image().flags();
            match ty {
                Dt::CombinedImageSampler | Dt::SampledImage =>
                    assert!(!flags.contains(ImageFlags::NO_SAMPLE)),
                Dt::StorageImage =>
                    assert!(flags.contains(ImageFlags::STORAGE)),
                Dt::InputAttachment =>
                    assert!(flags.contains(ImageFlags::INPUT_ATTACHMENT)),
                _ => unreachable!(),
            }
            match ty {
                Dt::CombinedImageSampler | Dt::SampledImage =>
                    assert_eq!(layout, Il::SHADER_READ_ONLY_OPTIMAL),
                Dt::StorageImage => assert_eq!(layout, Il::GENERAL),
                _ => {},
            }
            if ty == Dt::CombinedImageSampler
                && layout_binding.samplers.is_none()
            {
                    assert!(!sampler.is_null());
            }
        }

        let info = [vk::DescriptorImageInfo {
            sampler,
            image_view: view.inner(),
            image_layout: layout,
        }];
        let writes = [vk::WriteDescriptorSet {
            dst_set: self.inner(),
            dst_binding: binding,
            dst_array_element: element,
            descriptor_count: info.len() as _,
            descriptor_type: ty.into(),
            p_image_info: info.as_ptr(),
            ..Default::default()
        }];
        dt.update_descriptor_sets
            (writes.len() as _, writes.as_ptr(), 0, ptr::null());
    }

    pub unsafe fn write_samplers(
        &mut self,
        _binding: u32,
        _first_element: u32,
        _samplers: &[&Arc<Sampler>],
    ) {
        todo!()
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        let name: String = name.into();
        self.name = Some(name.clone());
        unsafe { self.device().set_name(self.inner(), name); }
    }
}

impl Named for Set {
    fn name(&self) -> Option<&str> {
        Some(&self.name.as_ref()?)
    }
}
