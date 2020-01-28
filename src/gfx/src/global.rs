use std::sync::Arc;

use crate::*;

#[derive(Debug)]
crate struct Globals {
    crate shaders: GlobalShaders,
    crate screen_pass: Arc<ScreenPass>,
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
    crate fn new(state: Arc<SystemState>) -> Self {
        unsafe { Self::unsafe_new(state) }
    }

    unsafe fn unsafe_new(state: Arc<SystemState>) -> Self {
        let device = Arc::clone(&state.device);

        let shaders = GlobalShaders::new(&device);
        let screen_pass = Arc::new(ScreenPass::new(Arc::clone(&device)));

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
            Arc::clone(&state),
            Default::default(),
            ImageType::TwoDim,
            Format::RGBA8,
            SampleCount::One,
            (1, 1).into(),
            1,
            1,
        )).create_full_view();
        let empty_storage_image_2d = Arc::new(Image::new(
            Arc::clone(&state),
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
            screen_pass,
            empty_vertex_layout,
            empty_uniform_buffer,
            empty_storage_buffer,
            empty_image_2d,
            empty_storage_image_2d,
            empty_sampler,
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

    fn smoke_test(vars: testing::TestVars) {
        let state = Arc::new(SystemState::new(Arc::clone(vars.device())));
        let _ = Globals::new(state);
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
