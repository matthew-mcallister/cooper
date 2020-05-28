use std::sync::Arc;

use enum_map::{Enum, EnumMap};

use crate::*;

mod simple;

crate use simple::*;

/// An identifier of a particular material rendering technique.
// TODO: Should be serializable to/from a string.
#[derive(Clone, Copy, Debug, Enum, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum MaterialProgram {
    Checker,
    FragDepth,
    FragNormal,
    // etc.
}

#[derive(Clone, Copy, Debug, Enum, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum MaterialImage {
    Albedo,
    Normal,
    MetallicRoughness,
    // etc.
}

pub type MaterialImageMap = EnumMap<MaterialImage, Option<Arc<ImageView>>>;

// TODO: Maybe make this a trait
#[derive(Debug)]
pub struct Material {
    crate renderer: Arc<dyn MaterialFactory>,
    crate program: MaterialProgram,
    crate images: MaterialImageMap,
    // Some material types
    crate desc: Option<DescriptorSet>,
}

crate trait MaterialFactory: std::fmt::Debug + Send + Sync {
    fn create_descriptor_set(&self, images: &MaterialImageMap) ->
        Option<DescriptorSet>;

    // TODO: Is this a sign that this abstraction is not all that good?
    fn select_shaders(&self, skinned: bool) -> ShaderStageMap;
}

impl Material {
    crate fn select_shaders(&self, skinned: bool) -> ShaderStageMap {
        self.renderer.select_shaders(skinned)
    }
}

#[derive(Debug)]
crate struct MaterialSystem {
    materials: EnumMap<MaterialProgram, Arc<dyn MaterialFactory>>,
}

impl MaterialSystem {
    crate fn new(_state: &SystemState, globals: &Arc<Globals>) -> Self {
        let [checker, depth, normal] =
            SimpleMaterialFactory::new(_state, globals);
        let materials = unsafe { std::mem::transmute([
             Arc::new(checker),  // Checker
             Arc::new(depth),    // FragDepth
             Arc::new(normal),   // FragNormal
        ]: [Arc<dyn MaterialFactory>; 3]) };
        Self {
            materials,
        }
    }

    crate fn create_material(
        &self,
        program: MaterialProgram,
        images: MaterialImageMap,
    ) -> Arc<Material> {
        let renderer = Arc::clone(&self.materials[program]);
        let desc = renderer.create_descriptor_set(&images);
        Arc::new(Material {
            renderer,
            program,
            images,
            desc,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use super::*;

    fn simple_test(vars: crate::testing::TestVars) {
        let device = vars.device();
        let state = SystemState::new(Arc::clone(&device));
        let globals = Arc::new(Globals::new(&state));
        let materials = MaterialSystem::new(&state, &globals);
        let _mat = materials.create_material(
            MaterialProgram::Checker, Default::default());
    }

    unit::declare_tests![
        simple_test,
    ];
}

unit::collect_tests![tests];
