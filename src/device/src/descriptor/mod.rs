use base::EnumVector;
use derivative::Derivative;
use enum_map::Enum;

use self::{
    DescriptorCounts as Counts, DescriptorHeap as Heap, DescriptorPool as Pool,
    DescriptorSet as Set, DescriptorSetLayout as Layout,
};

// Perhaps these should always go by the short name
pub use self::{
    DescriptorSetLayout as SetLayout, DescriptorSetLayoutDesc as SetLayoutDesc,
    DescriptorSetLayoutBinding as SetLayoutBinding,
    DescriptorSetLayoutCache as SetLayoutCache,
};

mod layout;
mod pool;
mod set;

pub use layout::*;
pub use pool::*;
pub use set::*;

wrap_vk_enum! {
    #[derive(Derivative, Enum)]
    #[derivative(Default)]
    pub enum DescriptorType {
        #[derivative(Default)]
        Sampler = SAMPLER,
        CombinedImageSampler = COMBINED_IMAGE_SAMPLER,
        SampledImage = SAMPLED_IMAGE,
        StorageImage = STORAGE_IMAGE,
        UniformBuffer = UNIFORM_BUFFER,
        StorageBuffer = STORAGE_BUFFER,
        InputAttachment = INPUT_ATTACHMENT,
    }
}

type DescriptorCounts = EnumVector<DescriptorType, u32>;

impl DescriptorType {
    #[inline]
    pub fn is_buffer(self) -> bool {
        match self {
            Self::UniformBuffer | Self::StorageBuffer => true,
            _ => false,
        }
    }

    #[inline]
    pub fn is_image(self) -> bool {
        match self {
            Self::CombinedImageSampler | Self::SampledImage
                | Self::StorageImage | Self::InputAttachment => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use vk::traits::*;
    use crate::*;
    use crate::testing::*;
    use super::*;

    unsafe fn alloc(vars: TestVars) {
        let device = vars.device();

        let (max_sets, descriptor_counts) = frame_descriptor_counts();
        let mut pool = DescriptorPool::new(
            Arc::clone(&device),
            max_sets,
            descriptor_counts,
            Lifetime::Static,
        );

        let constant_buffer_layout = Arc::new(SetLayout::new(
            Arc::clone(device),
            set_layout_desc![
                (0, StorageBuffer, VERTEX_BIT | FRAGMENT_BIT),
            ],
        ));
        let material_layout = Arc::new(SetLayout::new(
            Arc::clone(device),
            set_layout_desc![
                (0, CombinedImageSampler[3], FRAGMENT_BIT),
            ],
        ));

        let set0 = pool.alloc(&constant_buffer_layout);
        let sets = pool.alloc_many(&material_layout, 3);

        let used = pool.used_descriptors();
        assert_eq!(pool.used_sets(), 4);
        assert_eq!(used[DescriptorType::StorageBuffer], 1);
        assert_eq!(used[DescriptorType::CombinedImageSampler], 9);

        assert!(!sets.iter().any(|set| set.inner().is_null()));
        assert_ne!(sets[0].inner(), sets[1].inner());
        assert_ne!(sets[1].inner(), sets[2].inner());
        assert_ne!(sets[2].inner(), sets[0].inner());

        pool.free(&set0);
        let used = pool.used_descriptors();
        assert_eq!(pool.used_sets(), 3);
        assert_eq!(used[DescriptorType::StorageBuffer], 0);
    }

    unsafe fn write(vars: TestVars) {
        let device = Arc::clone(vars.device());
        let resources = TestResources::new(&device);
        let descriptors = &resources.descriptors;

        let layout = Arc::new(SetLayout::new(device, set_layout_desc![
            (0, UniformBuffer[2]),
            (1, SampledImage),
            (2, CombinedImageSampler[2]),
        ]));

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

    fn layout_zero_count(vars: TestVars) {
        SetLayout::new(Arc::clone(vars.device()), set_layout_desc![
            (0, UniformBuffer[0]),
        ]);
    }

    fn layout_duplicate_binding(vars: TestVars) {
        SetLayout::new(Arc::clone(vars.device()), set_layout_desc![
            (0, UniformBuffer),
            (0, UniformBuffer),
        ]);
    }

    fn layout_unordered_bindings(vars: TestVars) {
        SetLayout::new(Arc::clone(vars.device()), set_layout_desc![
            (1, UniformBuffer),
            (0, UniformBuffer),
        ]);
    }

    fn layout_cache(vars: TestVars) {
        let mut cache = SetLayoutCache::new(Arc::clone(vars.device()));
        let desc = set_layout_desc![
            (0, StorageBuffer, VERTEX_BIT | FRAGMENT_BIT),
        ];
        let layout: Arc<_> = cache.get_or_create(&desc).into_owned();
        assert_eq!(layout.desc(), &desc);
        let layout1 = cache.get_or_create(&desc);
        assert_eq!(&*layout as *const _, &**layout1 as *const _);
        cache.commit();
        let layout2 = cache.get_committed(&desc).unwrap();
        assert_eq!(&*layout as *const _, &**layout2 as *const _);

        let desc = set_layout_desc![
            (0, SampledImage, FRAGMENT_BIT),
        ];
        let layout: Arc<_> = cache.get_or_create(&desc).into_owned();
        assert_eq!(layout.desc(), &desc);
        let layout1 = cache.get_or_create(&desc);
        assert_eq!(&*layout as *const _, &**layout1 as *const _);
    }

    unit::declare_tests![
        alloc,
        write,
        (#[should_err] layout_zero_count),
        (#[should_err] layout_duplicate_binding),
        (#[should_err] layout_unordered_bindings),
        layout_cache,
    ];
}

unit::collect_tests![tests];
