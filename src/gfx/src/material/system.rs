use std::sync::Arc;

use base::partial_map;
use device::{ShaderSpec, ShaderStage, ShaderStageMap};

use crate::{Globals, GlobalShaders, ShaderConst};
use super::*;

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
#[non_exhaustive]
enum GeomVisMode {
    Checker = 0,
    Depth = 1,
    Normal = 2,
}

pub(super) fn create_def(
    state: &mut SystemState,
    globals: &Arc<Globals>,
    desc: Arc<MaterialDesc>,
) -> Arc<MaterialDef> {
    let stages = shader_stages(&globals, desc.program);
    let set_layout = create_set_layout(state, globals, &desc.image_bindings);
    Arc::new(MaterialDef { desc, stages, set_layout })
}

fn shader_stages(globals: &Globals, prog: MaterialProgram) -> ShaderStageMap {
    use MaterialProgram::*;
    let shaders = &globals.shaders;
    match prog {
        Checker => geom_vis_stages(shaders, GeomVisMode::Checker),
        GeomDepth => geom_vis_stages(shaders, GeomVisMode::Depth),
        GeomNormal => geom_vis_stages(shaders, GeomVisMode::Normal),
        Albedo => texture_vis_stages(shaders, MaterialImage::Albedo),
        NormalMap => texture_vis_stages(shaders, MaterialImage::Normal),
        MetallicRoughness =>
            texture_vis_stages(shaders, MaterialImage::MetallicRoughness),
    }
}

fn texture_vis_stages(shaders: &GlobalShaders, slot: MaterialImage) ->
    ShaderStageMap
{
    // TODO: This could easily be made into a macro. Or a function
    // taking an iterator. Or, better yet, ShaderSpec could just
    // accept a hashmap as input.
    let specialize = |shader| {
        let mut spec = ShaderSpec::new(Arc::clone(shader));
        spec.set(ShaderConst::TextureVisSlot as _, &(slot as u32));
        Arc::new(spec)
    };
    partial_map! {
        ShaderStage::Vertex => specialize(&shaders.static_vert),
        ShaderStage::Fragment => specialize(&shaders.texture_vis_frag),
    }
}

fn geom_vis_stages(shaders: &GlobalShaders, mode: GeomVisMode) ->
    ShaderStageMap
{
    let specialize = |shader| {
        let mut spec = ShaderSpec::new(Arc::clone(shader));
        spec.set(ShaderConst::GeomVisMode as _, &(mode as u32));
        Arc::new(spec)
    };
    partial_map! {
        ShaderStage::Vertex => specialize(&shaders.static_vert),
        ShaderStage::Fragment => specialize(&shaders.geom_vis_frag),
    }
}
