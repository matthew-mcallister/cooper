use std::sync::Arc;

use base::PartialEnumMap;
use device::{
    DescriptorSetLayout, Image, ImageDef, ImageSubresources, ImageViewFlags,
    SamplerDesc, ShaderStageMap, VertexInputLayout,
};
use enum_map::Enum;

use crate::SystemState;

mod state;
mod system;

crate use state::*;
use system::*;

/// An identifier of a particular material rendering technique.
// TODO: Should be serializable to/from a string.
#[derive(Clone, Copy, Debug, Enum, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum MaterialProgram {
    Checker,
    GeomDepth,
    GeomNormal,
    Albedo,
    NormalMap,
    MetallicRoughness,
}

#[derive(Clone, Copy, Debug, Enum, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum MaterialImage {
    Albedo = 0,
    Normal = 1,
    MetallicRoughness = 2,
}

// TODO: Probably should be called MaterialTextureDesc or something
#[derive(Clone, Debug)]
pub struct ImageBindingDesc {
    pub image: Arc<ImageDef>,
    // TODO: Should not be exposed to the user
    pub flags: ImageViewFlags,
    pub subresources: ImageSubresources,
    pub sampler_state: SamplerDesc,
}

pub type MaterialImageBindings =
    PartialEnumMap<MaterialImage, ImageBindingDesc>;

// TODO: Bake VkPipeline object(s) at material creation time.
// TODO: Allow descriptor set layout to be customized somewhat?
// TODO: Hash and memoized
#[derive(Debug)]
pub struct MaterialDef {
    vertex_layout: VertexInputLayout,
    program: MaterialProgram,
    stages: ShaderStageMap,
    set_layout: Arc<DescriptorSetLayout>,
    image_bindings: MaterialImageBindings,
}

impl MaterialDef {
    pub fn program(&self) -> MaterialProgram {
        self.program
    }

    pub fn shader_stages(&self) -> &ShaderStageMap {
        &self.stages
    }

    pub fn image_bindings(&self) -> &MaterialImageBindings {
        &self.image_bindings
    }

    fn set_layout(&self) -> &Arc<DescriptorSetLayout> {
        &self.set_layout
    }
}

impl MaterialImage {
    const SIZE: usize = <Self as Enum<()>>::POSSIBLE_VALUES;

    #[allow(dead_code)]
    crate fn values() -> impl ExactSizeIterator<Item = Self> {
        (0..Self::SIZE).map(<Self as Enum<()>>::from_usize)
    }
}
