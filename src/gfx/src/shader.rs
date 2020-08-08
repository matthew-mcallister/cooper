use std::sync::Arc;

use device::{Device, Shader, ShaderSpec};

use crate::material::MaterialImage;

#[derive(Debug)]
#[non_exhaustive]
pub struct GlobalShaders {
    pub trivial_vert: Arc<Shader>,
    pub trivial_frag: Arc<Shader>,
    pub static_vert: Arc<Shader>,
    pub geom_vis_frag: Arc<Shader>,
    pub tex_vis_frag: Arc<Shader>,
}

#[derive(Debug)]
#[non_exhaustive]
pub struct GlobalShaderSpecs {
    pub checker_frag: Arc<ShaderSpec>,
    pub geom_depth_frag: Arc<ShaderSpec>,
    pub geom_normal_frag: Arc<ShaderSpec>,
    pub albedo_frag: Arc<ShaderSpec>,
    pub tex_normal_frag: Arc<ShaderSpec>,
    pub metal_rough_frag: Arc<ShaderSpec>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ShaderConst {
    GeomVisMode = 0,
    TextureVisSlot = 1,
}

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeomVisMode {
    Checker = 0,
    Depth = 1,
    Normal = 2,
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
        tex_vis_frag = "tex_vis_frag";
    }
}

impl GlobalShaders {
    crate unsafe fn new(device: &Arc<Device>) -> Self {
        macro_rules! build {
            ($($name:ident,)*) => {
                Self {
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
            tex_vis_frag,
        }
    }
}

impl GlobalShaderSpecs {
    crate fn new(shaders: &GlobalShaders) -> Self {
        let spec_geom = |mode: GeomVisMode| {
            let mut spec = ShaderSpec::new(Arc::clone(&shaders.geom_vis_frag));
            spec.set(ShaderConst::GeomVisMode as _, &(mode as u32));
            Arc::new(spec)
        };
        let spec_tex = |slot: MaterialImage| {
            let mut spec = ShaderSpec::new(Arc::clone(&shaders.tex_vis_frag));
            spec.set(ShaderConst::TextureVisSlot as _, &(slot as u32));
            Arc::new(spec)
        };
        Self {
            checker_frag: spec_geom(GeomVisMode::Checker),
            geom_depth_frag: spec_geom(GeomVisMode::Depth),
            geom_normal_frag: spec_geom(GeomVisMode::Normal),
            albedo_frag: spec_tex(MaterialImage::Albedo),
            tex_normal_frag: spec_tex(MaterialImage::Normal),
            metal_rough_frag: spec_tex(MaterialImage::MetallicRoughness),
        }
    }
}
