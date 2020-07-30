use self::{
    DescriptorCounts as Counts, DescriptorHeap as Heap, DescriptorPool as Pool,
    DescriptorSet as Set, DescriptorSetLayout as Layout,
};

crate use self::DescriptorSetLayout as SetLayout;

mod count;
mod layout;
mod pool;
mod set;

crate use count::*;
crate use layout::*;
crate use pool::*;
crate use set::*;

fn is_valid_type(ty: vk::DescriptorType) -> bool {
    // Not supported yet: texel buffers, dynamic buffers
    [
        vk::DescriptorType::SAMPLER,
        vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        vk::DescriptorType::SAMPLED_IMAGE,
        vk::DescriptorType::STORAGE_IMAGE,
        vk::DescriptorType::UNIFORM_BUFFER,
        vk::DescriptorType::STORAGE_BUFFER,
        vk::DescriptorType::INPUT_ATTACHMENT,
    ].contains(&ty)
}

fn is_buffer(ty: vk::DescriptorType) -> bool {
    (ty == vk::DescriptorType::UNIFORM_BUFFER)
        | (ty == vk::DescriptorType::STORAGE_BUFFER)
}

fn is_uniform_buffer(ty: vk::DescriptorType) -> bool {
    ty == vk::DescriptorType::UNIFORM_BUFFER
}

fn is_storage_buffer(ty: vk::DescriptorType) -> bool {
    ty == vk::DescriptorType::STORAGE_BUFFER
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use vk::traits::*;
    use crate::*;
    use crate::testing::*;
    use super::*;

    unsafe fn constant_buffer_layout(device: &Arc<Device>) ->
        Arc<DescriptorSetLayout>
    {
        let device = Arc::clone(device);
        let bindings = [
            vk::DescriptorSetLayoutBinding {
                binding: 0,
                descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::VERTEX_BIT
                    | vk::ShaderStageFlags::FRAGMENT_BIT,
                ..Default::default()
            },
        ];
        DescriptorSetLayout::from_bindings(device, &bindings).into()
    }

    unsafe fn material_layout(device: &Arc<Device>) ->
        Arc<DescriptorSetLayout>
    {
        let device = Arc::clone(device);
        let bindings = [vk::DescriptorSetLayoutBinding {
            binding: 0,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 3,
            stage_flags: vk::ShaderStageFlags::FRAGMENT_BIT,
            ..Default::default()
        }];
        DescriptorSetLayout::from_bindings(device, &bindings).into()
    }

    unsafe fn alloc_test(vars: TestVars) {
        let device = vars.device();

        let (max_sets, descriptor_counts) = frame_descriptor_counts();
        let mut pool = DescriptorPool::new(
            Arc::clone(&device),
            max_sets,
            descriptor_counts,
            Lifetime::Static,
        );
        let constant_buffer_layout = constant_buffer_layout(&device);
        let material_layout = material_layout(&device);

        let set0 = pool.alloc(&constant_buffer_layout);
        let sets = pool.alloc_many(&material_layout, 3);

        let used = pool.used_descriptors();
        assert_eq!(pool.used_sets(), 4);
        assert_eq!(used[vk::DescriptorType::STORAGE_BUFFER], 1);
        assert_eq!(used[vk::DescriptorType::COMBINED_IMAGE_SAMPLER], 9);

        assert!(!sets.iter().any(|set| set.inner().is_null()));
        assert_ne!(sets[0].inner(), sets[1].inner());
        assert_ne!(sets[1].inner(), sets[2].inner());
        assert_ne!(sets[2].inner(), sets[0].inner());

        pool.free(&set0);
        let used = pool.used_descriptors();
        assert_eq!(pool.used_sets(), 3);
        assert_eq!(used[vk::DescriptorType::STORAGE_BUFFER], 0);
    }

    unsafe fn write_test(vars: TestVars) {
        let device = vars.device();
        let resources = TestResources::new(&device);
        let descriptors = &resources.descriptors;

        let bindings = set_layout_bindings![
            (0, UNIFORM_BUFFER[2]),
            (1, SAMPLED_IMAGE),
            (2, COMBINED_IMAGE_SAMPLER[2]),
        ];
        let layout = Arc::new(SetLayout::from_bindings(
            Arc::clone(&device), &bindings));

        let mut desc = descriptors.alloc(Lifetime::Static, &layout);
        let buffers = vec![resources.empty_uniform_buffer.range(); 2];
        let layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        desc.write_buffers(0, 0, &buffers);
        desc.write_image(1, &resources.empty_image_2d, layout, None);
        desc.write_images(
            2,
            0,
            &vec![&resources.empty_image_2d; 2],
            layout,
            Some(&vec![&resources.empty_sampler; 2]),
        );
    }

    unit::declare_tests![
        alloc_test,
        write_test,
    ];
}

unit::collect_tests![tests];
