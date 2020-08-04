use std::sync::Arc;

use base::PartialEnumMap;
use derivative::Derivative;
use device::{
    DescriptorSetLayout, Image, ImageDef, ImageSubresources, SamplerDesc,
    ShaderStageMap, VertexInputLayout,
};
use enum_map::Enum;

use crate::SystemState;
use crate::util::{ptr_eq, ptr_hash};

mod state;
mod system;

crate use state::*;
use system::*;

/// An identifier of a particular material rendering technique.
// TODO: Should be serializable to/from a string.
#[derive(Clone, Copy, Debug, Derivative, Enum, Eq, Hash, PartialEq)]
#[derivative(Default)]
#[non_exhaustive]
pub enum MaterialProgram {
    #[derivative(Default)]
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
#[derive(Clone, Debug, Derivative)]
#[derivative(Hash, PartialEq)]
pub struct ImageBindingDesc {
    #[derivative(Hash(hash_with = "ptr_hash"))]
    #[derivative(PartialEq(compare_with = "ptr_eq"))]
    pub image: Arc<ImageDef>,
    pub subresources: ImageSubresources,
    pub sampler_state: SamplerDesc,
}
impl Eq for ImageBindingDesc {}

pub type MaterialImageBindings =
    PartialEnumMap<MaterialImage, ImageBindingDesc>;

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct MaterialDesc {
    pub vertex_layout: VertexInputLayout,
    pub program: MaterialProgram,
    pub image_bindings: MaterialImageBindings,
}

// TODO: Allow descriptor set layout to be customized somewhat?
#[derive(Debug)]
pub struct MaterialDef {
    desc: Arc<MaterialDesc>,
    stages: ShaderStageMap,
    set_layout: Arc<DescriptorSetLayout>,
}

impl MaterialDef {
    fn vertex_layout(&self) -> &VertexInputLayout {
        &self.desc.vertex_layout
    }

    #[allow(dead_code)]
    fn program(&self) -> MaterialProgram {
        self.desc.program
    }

    fn image_bindings(&self) -> &MaterialImageBindings {
        &self.desc.image_bindings
    }

    fn stages(&self) -> &ShaderStageMap {
        &self.stages
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
