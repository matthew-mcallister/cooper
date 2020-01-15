use std::sync::Arc;

use crate::*;

#[derive(Debug)]
crate struct Globals {
    crate screen_pass: Arc<ScreenPass>,
    crate empty_vertex_layout: Arc<VertexLayout>,
    crate scene_global_layout: Arc<DescriptorSetLayout>,
    // TODO: Null resources/descriptors
    crate shaders: GlobalShaders,
}

#[derive(Debug)]
crate struct GlobalShaders {
    crate trivial_vert: Arc<Shader>,
    crate trivial_frag: Arc<Shader>,
}

impl Globals {
    crate fn new(device: Arc<Device>) -> Self {
        unsafe { Self::unsafe_new(device) }
    }

    unsafe fn unsafe_new(device: Arc<Device>) -> Self {
        let shaders = GlobalShaders::new(&device);

        let screen_pass = Arc::new(ScreenPass::new(Arc::clone(&device)));

        let empty_vertex_layout = Arc::new(VertexLayout {
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            bindings: Default::default(),
            attrs: Default::default(),
        });

        let bindings = set_layout_bindings![(0, UNIFORM_BUFFER)];
        let scene_global_layout =
            Arc::new(SetLayout::from_bindings(Arc::clone(&device), &bindings));

        Globals {
            screen_pass,
            empty_vertex_layout,
            scene_global_layout,
            shaders,
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
    // TODO: Load from disk, hot reloading
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
                location(0) VEC4 Color;
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
    use enum_map::enum_map;
    use crate::*;
    use super::*;

    fn smoke_test(vars: testing::TestVars) {
        let device = Arc::clone(vars.swapchain.device());
        let _ = Globals::new(device);
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
