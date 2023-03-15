use std::sync::Arc;

use device::{
    BufferRange, DescriptorHeap, DescriptorSet, DescriptorSetLayoutBinding,
    DescriptorSetLayoutCache, DescriptorType, ImageView, Lifetime, Sampler,
};

#[derive(Debug)]
pub enum DescriptorResource<'r> {
    UniformBuffers(&'r [BufferRange<'r>], vk::ShaderStageFlags),
    StorageBuffers(&'r [BufferRange<'r>], vk::ShaderStageFlags),
    // Will be images, samplers, or combined image samplers depending on
    // what resources are specified.
    ImageSamplers {
        images: &'r [(&'r ImageView, vk::ImageLayout)],
        samplers: &'r [&'r Arc<Sampler>],
        stage_flags: vk::ShaderStageFlags,
        immutable_samplers: bool,
    },
    // TODO: Input attachments
}

// TODO?: Support unused bindings
pub(crate) fn create_descriptor_set<'r>(
    layout_cache: &DescriptorSetLayoutCache,
    heap: &Arc<DescriptorHeap>,
    lifetime: Lifetime,
    name: Option<String>,
    resources: &[DescriptorResource<'r>],
) -> DescriptorSet {
    let bindings = resources
        .iter()
        .enumerate()
        .map(|(i, res)| match res {
            DescriptorResource::UniformBuffers(ranges, flags) => DescriptorSetLayoutBinding {
                binding: i as _,
                ty: DescriptorType::UniformBuffer,
                count: ranges.len() as _,
                stage_flags: *flags,
                samplers: None,
            },
            DescriptorResource::StorageBuffers(ranges, flags) => DescriptorSetLayoutBinding {
                binding: i as _,
                ty: DescriptorType::StorageBuffer,
                count: ranges.len() as _,
                stage_flags: *flags,
                samplers: None,
            },
            &DescriptorResource::ImageSamplers {
                ref images,
                ref samplers,
                stage_flags,
                immutable_samplers,
            } => {
                let ty = match (images.is_empty(), samplers.is_empty()) {
                    (false, false) => {
                        assert_eq!(images.len(), samplers.len());
                        DescriptorType::CombinedImageSampler
                    }
                    (true, false) => DescriptorType::Sampler,
                    (false, true) => DescriptorType::SampledImage,
                    (true, true) => panic!("Empty image descriptor binding"),
                };
                let count = (images.len() | samplers.len()) as u32;
                let samplers = if immutable_samplers {
                    Some(samplers.iter().map(|s| Arc::clone(s)).collect())
                } else {
                    None
                };
                DescriptorSetLayoutBinding {
                    binding: i as _,
                    ty,
                    count,
                    stage_flags: stage_flags,
                    samplers,
                }
            }
        })
        .collect();
    let desc = device::DescriptorSetLayoutDesc { bindings };
    let layout = layout_cache.get_or_create_named(&desc, name);
    let mut set = heap.alloc(lifetime, &layout);
    // TODO: Prooobably actually fully support DescriptorSet::update
    // instead of making several separate calls.
    for (i, res) in resources.iter().enumerate() {
        match res {
            DescriptorResource::UniformBuffers(ranges, _) => {
                set.write_buffers(i as _, 0, ranges);
            }
            DescriptorResource::StorageBuffers(ranges, _) => {
                set.write_buffers(i as _, 0, ranges);
            }
            &DescriptorResource::ImageSamplers {
                ref images,
                ref samplers,
                ..
            } => {
                let samplers: Vec<_> = samplers.iter().map(|s| &***s).collect();
                set.write_image_samplers(i as _, 0, images, &samplers[..]);
            }
        }
    }
    set
}
