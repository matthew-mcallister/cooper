use std::sync::Arc;

use device::{
    DescriptorHeap, DescriptorSet, DescriptorSetLayoutBinding, DescriptorSetLayoutCache,
    DescriptorType,
};

#[derive(Debug)]
pub enum DescriptorResource<'r> {
    UniformBuffer(device::BufferRange<'r>, vk::ShaderStageFlags),
    UniformBufferArray(&'r [device::BufferRange<'r>], vk::ShaderStageFlags),
    StorageBuffer(device::BufferRange<'r>, vk::ShaderStageFlags),
    StorageBufferArray(&'r [device::BufferRange<'r>], vk::ShaderStageFlags),
    // TODO: Need to handle samplers
    //Image(&'r device::ImageView, u32),
}

pub(crate) fn create_descriptor_set<'r>(
    layout_cache: &DescriptorSetLayoutCache,
    heap: &Arc<DescriptorHeap>,
    lifetime: device::Lifetime,
    name: Option<String>,
    resources: &[DescriptorResource<'r>],
) -> DescriptorSet {
    let bindings = resources
        .iter()
        .enumerate()
        .map(|(i, res)| match res {
            DescriptorResource::UniformBuffer(_, flags) => DescriptorSetLayoutBinding {
                binding: i as _,
                ty: DescriptorType::UniformBuffer,
                count: 1,
                stage_flags: *flags,
                samplers: None,
            },
            DescriptorResource::UniformBufferArray(ranges, flags) => DescriptorSetLayoutBinding {
                binding: i as _,
                ty: DescriptorType::UniformBuffer,
                count: ranges.len() as _,
                stage_flags: *flags,
                samplers: None,
            },
            DescriptorResource::StorageBuffer(_, flags) => DescriptorSetLayoutBinding {
                binding: i as _,
                ty: DescriptorType::StorageBuffer,
                count: 1,
                stage_flags: *flags,
                samplers: None,
            },
            DescriptorResource::StorageBufferArray(ranges, flags) => DescriptorSetLayoutBinding {
                binding: i as _,
                ty: DescriptorType::StorageBuffer,
                count: ranges.len() as _,
                stage_flags: *flags,
                samplers: None,
            },
        })
        .collect();
    let desc = device::DescriptorSetLayoutDesc { bindings };
    let layout = layout_cache.get_or_create_named(&desc, name);
    let mut set = heap.alloc(lifetime, &layout);
    // TODO: Prooobably actually fully support DescriptorSet::update
    // instead of making many separate calls.
    for (i, res) in resources.iter().enumerate() {
        match res {
            DescriptorResource::UniformBuffer(range, _) => {
                set.write_buffer(i as _, *range);
            }
            DescriptorResource::UniformBufferArray(ranges, _) => {
                set.write_buffers(i as _, 0, ranges);
            }
            DescriptorResource::StorageBuffer(range, _) => {
                set.write_buffer(i as _, *range);
            }
            DescriptorResource::StorageBufferArray(ranges, _) => {
                set.write_buffers(i as _, 0, ranges);
            }
        }
    }
    set
}
