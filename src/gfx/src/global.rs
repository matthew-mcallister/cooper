use std::sync::Arc;

use log::trace;

use crate::*;

#[derive(Debug)]
crate struct Globals {
    crate device: Arc<Device>,
    crate shaders: GlobalShaders,
    crate empty_uniform_buffer: Arc<BufferAlloc>,
    crate empty_storage_buffer: Arc<BufferAlloc>,
    crate immediate_image_2d: Arc<ImageView>,
    crate immediate_storage_image_2d: Arc<ImageView>,
    crate empty_image_2d: Arc<ImageDef>,
    crate empty_sampler: Arc<Sampler>,
    crate scene_desc_layout: Arc<DescriptorSetLayout>,
}

#[derive(Debug)]
crate struct GlobalShaders {
    crate trivial_vert: Arc<Shader>,
    crate trivial_frag: Arc<Shader>,
    crate static_vert: Arc<Shader>,
    crate geom_vis_frag: Arc<Shader>,
    crate texture_vis_frag: Arc<Shader>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
crate enum ShaderConst {
    GeomVisMode = 0,
    TextureVisSlot = 1,
}

impl Globals {
    crate fn new(state: &SystemState, heap: &ImageHeap) -> Self {
        unsafe { Self::unsafe_new(state, heap) }
    }

    unsafe fn unsafe_new(state: &SystemState, heap: &ImageHeap) -> Self {
        let device = Arc::clone(&state.device);

        let shaders = GlobalShaders::new(&device);

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
            .build_image(heap)
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
            .build_image(heap)
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
        let empty_sampler = Arc::clone(&state.samplers.get_or_create(&desc));

        let scene_desc_layout =
            SceneDescriptors::create_layout(Arc::clone(&device));

        Globals {
            device,
            shaders,
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

    // TODO: This is obsolete
    crate unsafe fn write_empty_descriptors(&self, desc: &mut DescriptorSet) {
        let layout = Arc::clone(&desc.layout());
        for i in 0..layout.bindings().len() {
            self.write_empty_descriptors_binding(&layout, desc, i as _);
        }
    }

    crate unsafe fn write_empty_descriptors_binding(
        &self,
        layout: &Arc<SetLayout>,
        desc: &mut DescriptorSet,
        binding: u32,
    ) {
        use vk::DescriptorType as Dt;
        let layout_binding = &layout.bindings()[binding as usize];
        let count = layout_binding.descriptor_count as usize;
        match layout_binding.descriptor_type {
            Dt::SAMPLER => todo!(),
            Dt::COMBINED_IMAGE_SAMPLER => {
                let views = vec![&self.immediate_image_2d; count];
                let samplers = vec![&self.empty_sampler; count];
                let layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
                desc.write_images(binding, 0, &views, layout, Some(&samplers));
            },
            Dt::SAMPLED_IMAGE => {
                let views = vec![&self.immediate_image_2d; count];
                let layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
                desc.write_images(binding, 0, &views, layout, None);
            },
            Dt::STORAGE_IMAGE => {
                let views = vec![&self.immediate_storage_image_2d; count];
                let layout = vk::ImageLayout::GENERAL;
                desc.write_images(binding, 0, &views, layout, None);
            },
            Dt::UNIFORM_BUFFER => {
                let bufs = vec![self.empty_uniform_buffer.range(); count];
                desc.write_buffers(binding, 0, &bufs);
            },
            Dt::STORAGE_BUFFER => {
                let bufs = vec![self.empty_storage_buffer.range(); count];
                desc.write_buffers(binding, 0, &bufs);
            },
            _ => trace!("uninitialized descriptor: binding: {}, layout: {:?}",
                binding, layout),
        }
    }
}

mod shader_sources {
    macro_rules! include_shaders {
        ($($ident:ident = $name:expr;)*) => {
            $(crate mod $ident {
                macro_rules! name {
                    () => {
                        concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/generated/shaders/", $name, ".spv",
                        )
                    }
                }
                crate const NAME: &'static str = name!();
                crate static CODE: &'static [u32] = include_u32!(name!());
            })*
        }
    }

    include_shaders! {
        trivial_vert = "trivial_vert";
        trivial_frag = "trivial_frag";
        static_vert = "static_vert";
        geom_vis_frag = "geom_vis_frag";
        texture_vis_frag = "texture_vis_frag";
    }
}

impl GlobalShaders {
    crate unsafe fn new(device: &Arc<Device>) -> Self {
        macro_rules! build {
            ($($name:ident,)*) => {
                GlobalShaders {
                    $($name: Arc::new(Shader::new(
                        Arc::clone(&device),
                        shader_sources::$name::CODE.into(),
                        Some(shader_sources::$name::NAME.to_owned()),
                    )),)*
                }
            }
        }

        build! {
            trivial_vert,
            trivial_frag,
            static_vert,
            geom_vis_frag,
            texture_vis_frag,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let state = SystemState::new(Arc::clone(vars.device()));
        let heap = ImageHeap::new(Arc::clone(vars.device()));
        let _ = Globals::new(&state, &heap);
    }

    unsafe fn empty_descriptors_test(vars: testing::TestVars) {
        let device = Arc::clone(vars.device());
        let state = SystemState::new(Arc::clone(&device));
        let heap = ImageHeap::new(Arc::clone(&device));
        let globals = Globals::new(&state, &heap);

        let bindings = set_layout_bindings![
            (0, UNIFORM_BUFFER),
            (1, UNIFORM_BUFFER[2]),
            (2, STORAGE_BUFFER),
            (3, STORAGE_BUFFER[2]),
            (4, COMBINED_IMAGE_SAMPLER),
            (5, COMBINED_IMAGE_SAMPLER[2]),
            (6, SAMPLED_IMAGE),
            (7, SAMPLED_IMAGE[2]),
            (8, STORAGE_IMAGE),
            (9, STORAGE_IMAGE[2]),
        ];
        let layout = Arc::new(SetLayout::from_bindings(device, &bindings));

        let mut desc = state.descriptors.alloc(Lifetime::Frame, &layout);
        globals.write_empty_descriptors(&mut desc);
    }

    unit::declare_tests![
        smoke_test,
        empty_descriptors_test
    ];
}

unit::collect_tests![tests];
