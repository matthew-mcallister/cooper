use std::sync::Arc;

use device::*;

use crate::*;

#[derive(Debug)]
crate struct Globals {
    crate device: Arc<Device>,
    crate shaders: GlobalShaders,
    crate specs: GlobalShaderSpecs,
    crate empty_uniform_buffer: Arc<BufferAlloc>,
    crate empty_storage_buffer: Arc<BufferAlloc>,
    crate immediate_image_2d: Arc<ImageView>,
    crate immediate_storage_image_2d: Arc<ImageView>,
    crate empty_image_2d: Arc<ImageDef>,
    crate empty_sampler: Arc<Sampler>,
    crate scene_desc_layout: Arc<DescriptorSetLayout>,
}

impl Globals {
    crate fn new(state: &mut SystemState) -> Self {
        unsafe { Self::unsafe_new(state) }
    }

    unsafe fn unsafe_new(state: &mut SystemState) -> Self {
        let device = Arc::clone(&state.device);

        let shaders = GlobalShaders::new(&device);
        let specs = GlobalShaderSpecs::new(&shaders);

        // TODO: Probably should make sure these are filled with zero.
        let empty_uniform_buffer = Arc::new(state.buffers.alloc(
            BufferBinding::Uniform,
            Lifetime::Static,
            MemoryMapping::DeviceLocal,
            256,
        ));
        let empty_storage_buffer = Arc::new(state.buffers.alloc(
            BufferBinding::Storage,
            Lifetime::Static,
            MemoryMapping::DeviceLocal,
            256,
        ));

        let immediate_image_2d = ImageDef::new(
            &device,
            Default::default(),
            ImageType::Dim2,
            Format::RGBA8,
            SampleCount::One,
            (1, 1).into(),
            1,
            1,
        ).with_name("immediate_image_2d")
            .build_image(&state.image_heap)
            .create_full_view();

        let immediate_storage_image_2d = ImageDef::new(
            &device,
            ImageFlags::STORAGE | ImageFlags::NO_SAMPLE,
            ImageType::Dim2,
            Format::RGBA8,
            SampleCount::One,
            (1, 1).into(),
            1,
            1,
        ).with_name("immediate_storage_image_2d")
            .build_image(&state.image_heap)
            .create_full_view();

        let empty_image_2d = Arc::new(ImageDef::new(
            &device,
            Default::default(),
            ImageType::Dim2,
            Format::RGBA8,
            SampleCount::One,
            (1, 1).into(),
            1,
            1,
        ).with_name("empty_image_2d"));

        let desc = SamplerDesc {
            mag_filter: Filter::Linear,
            min_filter: Filter::Linear,
            mipmap_mode: SamplerMipmapMode::Linear,
            anisotropy_level: AnisotropyLevel::Sixteen,
            ..Default::default()
        };
        let empty_sampler =
            Arc::clone(&state.samplers.get_or_create_committed(&desc));

        let scene_desc_layout =
            SceneDescriptors::create_layout(&state.set_layouts);

        Globals {
            device,
            shaders,
            specs,
            empty_uniform_buffer,
            empty_storage_buffer,
            immediate_image_2d,
            immediate_storage_image_2d,
            empty_image_2d,
            empty_sampler,
            scene_desc_layout,
        }
    }

    crate fn upload_images(&self, resources: &mut ResourceSystem) {
        let image_data = Arc::new(vec![0, 0, 0, 0]);
        resources.upload_image(
            &self.empty_image_2d,
            Arc::clone(&image_data),
            0,
        );
    }
}
