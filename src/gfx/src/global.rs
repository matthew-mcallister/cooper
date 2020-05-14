use std::sync::Arc;

use log::trace;

use crate::*;

#[derive(Debug)]
crate struct Globals {
    crate device: Arc<Device>,
    crate shaders: GlobalShaders,
    crate empty_pipeline_layout: Arc<PipelineLayout>,
    crate empty_uniform_buffer: Arc<BufferAlloc>,
    crate empty_storage_buffer: Arc<BufferAlloc>,
    crate empty_image_2d: Arc<ImageView>,
    crate empty_storage_image_2d: Arc<ImageView>,
    crate empty_sampler: Arc<Sampler>,
    crate instance_buf_layout: Arc<DescriptorSetLayout>,
    crate scene_unifs_layout: Arc<DescriptorSetLayout>,
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

        let empty_pipeline_layout =
            Arc::new(PipelineLayout::new(Arc::clone(&device), vec![]));

        let empty_uniform_buffer = Arc::new(state.buffers.alloc(
            BufferBinding::Uniform,
            Lifetime::Static,
            MemoryMapping::Unmapped,
            256,
        ));
        let empty_storage_buffer = Arc::new(state.buffers.alloc(
            BufferBinding::Storage,
            Lifetime::Static,
            MemoryMapping::Unmapped,
            256,
        ));

        let empty_image_2d = Arc::new(Image::new(
            &state,
            Default::default(),
            ImageType::Dim2,
            Format::RGBA8,
            SampleCount::One,
            (1, 1).into(),
            1,
            1,
        )).create_full_view();
        let empty_storage_image_2d = Arc::new(Image::new(
            &state,
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

        let bindings = [
            vk::DescriptorSetLayoutBinding {
                binding: 0,
                descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::VERTEX_BIT
                    | vk::ShaderStageFlags::FRAGMENT_BIT,
                ..Default::default()
            },
        ];
        let scene_unifs_layout = Arc::new(DescriptorSetLayout::from_bindings(
            Arc::clone(&device), &bindings));
        let bindings = [
            vk::DescriptorSetLayoutBinding {
                binding: 0,
                descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::VERTEX_BIT,
                ..Default::default()
            },
        ];
        let instance_buf_layout = Arc::new(DescriptorSetLayout::from_bindings(
            Arc::clone(&device), &bindings));

        Globals {
            device,
            shaders,
            empty_pipeline_layout,
            empty_uniform_buffer,
            empty_storage_buffer,
            empty_image_2d,
            empty_storage_image_2d,
            empty_sampler,
            scene_unifs_layout,
            instance_buf_layout,
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
        DEBUG_DEPTH_FRAG = "simple_frag";
    }
}

impl GlobalShaders {
    unsafe fn new(device: &Arc<Device>) -> Self {
        let trivial_vert = Arc::new(Shader::new(
            Arc::clone(&device),
            shader_sources::TRIVIAL_VERT.to_vec(),
        ));
        let trivial_frag = Arc::new(Shader::new(
            Arc::clone(&device),
            shader_sources::TRIVIAL_FRAG.to_vec(),
        ));
        let static_vert = Arc::new(Shader::new(
            Arc::clone(&device),
            shader_sources::STATIC_VERT.to_vec(),
        ));
        let simple_frag = Arc::new(Shader::new(
            Arc::clone(&device),
            shader_sources::DEBUG_DEPTH_FRAG.to_vec(),
        ));
        GlobalShaders {
            trivial_vert,
            trivial_frag,
            static_vert,
            simple_frag,
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

        let mut desc = state.descriptors.alloc(&layout);
        globals.write_empty_descriptors(&mut desc);
    }

    unit::declare_tests![
        smoke_test,
        empty_descriptors_test
    ];
}

unit::collect_tests![tests];
