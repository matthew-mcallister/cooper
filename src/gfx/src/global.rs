use std::sync::Arc;

use crate::*;

#[derive(Debug)]
crate struct Globals {
    crate shaders: GlobalShaders,
    crate empty_vertex_layout: Arc<VertexLayout>,
    crate empty_uniform_buffer: Arc<BufferRange>,
    crate empty_storage_buffer: Arc<BufferRange>,
    crate empty_image_2d: Arc<ImageView>,
    crate empty_storage_image_2d: Arc<ImageView>,
    crate empty_sampler: Arc<Sampler>,
}

#[derive(Debug)]
crate struct GlobalShaders {
    crate trivial_vert: Arc<Shader>,
    crate trivial_frag: Arc<Shader>,
}

impl Globals {
    crate fn new(state: &SystemState) -> Self {
        unsafe { Self::unsafe_new(state) }
    }

    unsafe fn unsafe_new(state: &SystemState) -> Self {
        let device = Arc::clone(&state.device);

        let shaders = GlobalShaders::new(&device);

        let empty_vertex_layout = Arc::new(VertexLayout {
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            bindings: Default::default(),
            attrs: Default::default(),
        });

        // TODO: Manually acquiring this lock is so dumb
        let mut buffers = state.buffers.lock();
        let empty_uniform_buffer = Arc::new(buffers.alloc(
            BufferBinding::Uniform,
            MemoryMapping::Unmapped,
            256,
        ));
        let empty_storage_buffer = Arc::new(buffers.alloc(
            BufferBinding::Storage,
            MemoryMapping::Unmapped,
            256,
        ));
        std::mem::drop(buffers);

        let empty_image_2d = Arc::new(Image::new(
            &state,
            Default::default(),
            ImageType::TwoDim,
            Format::RGBA8,
            SampleCount::One,
            (1, 1).into(),
            1,
            1,
        )).create_full_view();
        let empty_storage_image_2d = Arc::new(Image::new(
            &state,
            ImageFlags::STORAGE,
            ImageType::TwoDim,
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

        Globals {
            shaders,
            empty_vertex_layout,
            empty_uniform_buffer,
            empty_storage_buffer,
            empty_image_2d,
            empty_storage_image_2d,
            empty_sampler,
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
                desc.write_images(binding, 0, &views, Some(&samplers));
            },
            Dt::SAMPLED_IMAGE => {
                let views = vec![&self.empty_image_2d; count];
                desc.write_images(binding, 0, &views, None);
            },
            Dt::STORAGE_IMAGE => {
                let views = vec![&self.empty_storage_image_2d; count];
                desc.write_images(binding, 0, &views, None);
            },
            Dt::UNIFORM_BUFFER => {
                let bufs = vec![&self.empty_uniform_buffer; count];
                desc.write_buffers(binding, 0, &bufs);
            },
            Dt::STORAGE_BUFFER => {
                let bufs = vec![&self.empty_storage_buffer; count];
                desc.write_buffers(binding, 0, &bufs);
            },
            // TODO: Input attachment?
            _ => unreachable!(),
        }
    }
}

#[allow(unused_macros)]
macro_rules! vertex_inputs {
    ($(location($loc:expr) $type:ident $Attr:ident;)*) => {
        [$(
            ShaderVar {
                location: $loc,
                ty: ShaderVarType::$type,
                attr: Some(VertexAttrName::$Attr),
                attch: None,
            },
        )*]
    }
}

macro_rules! fragment_outputs {
    ($(location($loc:expr) $type:ident $Attch:ident;)*) => {
        [$(
            ShaderVar {
                location: $loc,
                ty: ShaderVarType::$type,
                attr: None,
                attch: Some(AttachmentName::$Attch),
            },
        )*]
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
    }
}

impl GlobalShaders {
    unsafe fn new(device: &Arc<Device>) -> Self {
        let consts = [];
        let trivial_vert = Arc::new(Shader::new(
            Arc::clone(&device),
            shader_sources::TRIVIAL_VERT.to_vec(),
            ShaderStage::Vertex,
            Vec::new(),
            // Intermediates are ignored for now
            Vec::new(),
            consts.to_vec(),
        ));
        let trivial_frag = Arc::new(Shader::new(
            Arc::clone(&device),
            shader_sources::TRIVIAL_FRAG.to_vec(),
            ShaderStage::Fragment,
            Vec::new(),
            fragment_outputs! {
                location(0) VEC4 Backbuffer;
            }.to_vec(),
            consts.to_vec(),
        ));

        GlobalShaders {
            trivial_vert,
            trivial_frag,
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

        let mut descs = state.descriptors.lock();
        let mut desc = descs.alloc(&layout);
        globals.write_empty_descriptors(&mut desc);
    }

    unit::declare_tests![
        smoke_test,
        empty_descriptors_test
    ];
}

unit::collect_tests![tests];
