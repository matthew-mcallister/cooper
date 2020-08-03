use std::sync::Arc;

use base::partial_map;
use device::{ShaderSpec, ShaderStage, ShaderStageMap};

use crate::{Globals, GlobalShaders, ShaderConst, SystemState};
use crate::resource::ResourceSystem;
use super::*;

#[derive(Debug)]
crate struct MaterialSystem {
    globals: Arc<Globals>,
    materials: MaterialStateTable,
}

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
#[non_exhaustive]
enum GeomVisMode {
    Checker = 0,
    Depth = 1,
    Normal = 2,
}

impl MaterialSystem {
    crate fn new(_state: &SystemState, globals: &Arc<Globals>) -> Self {
        Self {
            globals: Arc::clone(globals),
            materials: MaterialStateTable::new(globals),
        }
    }

    crate fn define_material(
        &self,
        program: MaterialProgram,
        images: MaterialImageBindings,
    ) -> Arc<MaterialDef> {
        let stages = shader_stages(&self.globals, program);
        Arc::new(MaterialDef {
            program,
            stages,
            image_bindings: images,
        })
    }

    crate fn get_or_create(
        &mut self,
        state: &SystemState,
        resources: &ResourceSystem,
        def: &Arc<MaterialDef>,
    ) -> Result<&Arc<Material>, ResourceUnavailable> {
        self.materials.get_or_create(state, resources, def)
    }
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
