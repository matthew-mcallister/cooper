use std::sync::Arc;

use base::PartialEnumMap;
use enum_map::{Enum, EnumMap};

use crate::{
    DescriptorSet, Globals, Image, ImageSubresources, ImageView,
    ImageViewFlags, Sampler, ShaderStageMap, SystemState,
};

mod simple;
mod system;

use simple::*;
crate use system::*;

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

#[derive(Clone, Debug)]
pub struct ImageBindingDesc {
    // TODO: Maybe should create an "ImageViewDesc" type.
    pub image: Arc<Image>,
    pub flags: ImageViewFlags,
    pub subresources: ImageSubresources,
    pub sampler: Arc<Sampler>,
}

pub type MaterialImageBindings =
    PartialEnumMap<MaterialImage, ImageBindingDesc>;

#[derive(Clone, Debug)]
crate struct ImageBindingState {
    crate view: Arc<ImageView>,
    crate sampler: Arc<Sampler>,
}

crate type MaterialImageState =
    PartialEnumMap<MaterialImage, ImageBindingState>;

// TODO: Maybe make this a trait
// TODO: Bake VkPipeline object(s) into the material itself
#[derive(Debug)]
pub struct Material {
    crate renderer: Arc<dyn MaterialFactory>,
    crate program: MaterialProgram,
    crate images: PartialEnumMap<MaterialImage, ImageBindingState>,
    crate desc: Option<DescriptorSet>,
}

crate trait MaterialFactory: std::fmt::Debug + Send + Sync {
    fn create_descriptor_set(
        &self,
        state: &SystemState,
        globals: &Globals,
        images: &MaterialImageState,
    ) -> Option<DescriptorSet>;

    // TODO: Not necessary.
    fn select_shaders(&self) -> ShaderStageMap;
}

impl Material {
    crate fn select_shaders(&self) -> ShaderStageMap {
        self.renderer.select_shaders()
    }
}

impl MaterialImage {
    crate fn values() -> impl ExactSizeIterator<Item = Self> {
        (0..<Self as Enum<()>>::POSSIBLE_VALUES)
            .map(<Self as Enum<()>>::from_usize)
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
            &state, &globals,
            MaterialProgram::Checker, Default::default(),
        );
    }

    unit::declare_tests![
        simple_test,
    ];
}

unit::collect_tests![tests];
