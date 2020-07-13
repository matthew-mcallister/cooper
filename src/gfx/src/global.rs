use std::sync::Arc;

use log::trace;
use prelude::*;

use crate::*;

#[derive(Debug)]
crate struct Globals {
    crate device: Arc<Device>,
    crate shaders: GlobalShaders,
    crate empty_uniform_buffer: Arc<BufferAlloc>,
    crate empty_storage_buffer: Arc<BufferAlloc>,
    crate empty_image_2d: Arc<ImageView>,
    crate empty_storage_image_2d: Arc<ImageView>,
    crate empty_sampler: Arc<Sampler>,
    crate scene_desc_layout: Arc<DescriptorSetLayout>,
}

#[derive(Debug)]
crate struct GlobalShaders {
    crate trivial_vert: Arc<Shader>,
    crate trivial_frag: Arc<Shader>,
    crate static_vert: Arc<Shader>,
    crate simple_frag: Arc<Shader>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
crate enum ShaderConst {
    SimpleMode = 0,
}

impl Globals {
    crate fn new(state: &SystemState) -> Self {
        unsafe { Self::unsafe_new(state) }
    }

    unsafe fn unsafe_new(state: &SystemState) -> Self {
        let device = Arc::clone(&state.device);

        let shaders = GlobalShaders::new(&device);
        let heap = &state.heap;

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

        // TODO: These need layout transitions before use
        let empty_image_2d = Arc::new(Image::with(
            heap,
            Default::default(),
            ImageType::Dim2,
            Format::RGBA8,
            SampleCount::One,
            (1, 1).into(),
            1,
            1,
        )).create_full_view();
        let empty_storage_image_2d = Arc::new(Image::with(
            heap,
            ImageFlags::STORAGE,
            ImageType::Dim2,
            Format::RGBA8,
            SampleCount::One,
            (1, 1).into(),
            1,
            1,
        )).create_full_view();

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
            empty_image_2d,
            empty_storage_image_2d,
            empty_sampler,
            scene_desc_layout,
        }
    }

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
                let views = vec![&self.empty_image_2d; count];
                let samplers = vec![&self.empty_sampler; count];
                let layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
                desc.write_images(binding, 0, &views, layout, Some(&samplers));
            },
            Dt::SAMPLED_IMAGE => {
                let views = vec![&self.empty_image_2d; count];
                let layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
                desc.write_images(binding, 0, &views, layout, None);
            },
            Dt::STORAGE_IMAGE => {
                let views = vec![&self.empty_storage_image_2d; count];
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
    // TODO: Hot reloading
    macro_rules! include_shaders {
        ($($ident:ident = $name:expr;)*) => {
            $(crate const $ident: &'static [u8] = include_bytes!(
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/generated/shaders/", $name, ".spv",
                )
            );)*
        }
    }

    include_shaders! {
        TRIVIAL_VERT = "trivial_vert";
        TRIVIAL_FRAG = "trivial_frag";
        STATIC_VERT = "static_vert";
        SIMPLE_FRAG = "simple_frag";
    }
}

impl GlobalShaders {
    unsafe fn new(device: &Arc<Device>) -> Self {
        let to_words = |bytes: &[u8]| {
            assert_eq!(bytes.len() % 4, 0);
            let mut words = Vec::with_capacity(bytes.len() / 4);
            words.set_len(words.capacity());
            words.as_bytes_mut().copy_from_slice(bytes);
            words
        };

        macro_rules! build {
            ($($field:ident => $shader:ident,)*) => {
                GlobalShaders {
                    $($field: {
                        let shader = Arc::new(Shader::new(
                            Arc::clone(&device),
                            to_words(shader_sources::$shader),
                        ));
                        device.set_name(&*shader, stringify!($field));
                        shader
                    },)*
                }
            }
        }

        build! {
            trivial_vert => TRIVIAL_VERT,
            trivial_frag => TRIVIAL_FRAG,
            static_vert => STATIC_VERT,
            simple_frag => SIMPLE_FRAG,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let state = SystemState::new(Arc::clone(vars.device()));
        let _ = Globals::new(&state);
    }

    unsafe fn empty_descriptors_test(vars: testing::TestVars) {
        let device = Arc::clone(vars.device());
        let state = SystemState::new(Arc::clone(&device));
        let globals = Globals::new(&state);

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
