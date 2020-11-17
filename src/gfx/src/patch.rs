use std::sync::Arc;

use base::PartialEnumMap;
use derivative::Derivative;
use device::{
    CullMode, DescriptorSetLayout, ImageDef, SamplerDesc, ShaderSpec,
    ShaderStage, ShaderStageMap, VertexInputLayout,
};
use enum_map::Enum;

use crate::SystemState;
use crate::util::{ptr_eq, ptr_hash};

mod state;

crate use state::*;

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
    pub sampler_state: SamplerDesc,
}
impl Eq for ImageBindingDesc {}

pub type MaterialImageBindings =
    PartialEnumMap<MaterialImage, ImageBindingDesc>;

// TODO: Allow descriptor set layout to be customized somewhat?
#[derive(Debug)]
pub struct MaterialDef {
    desc: Arc<MaterialDesc>,
    set_layout: Arc<DescriptorSetLayout>,
}

impl MaterialDesc {
    pub fn vertex_stage(&self) -> Option<&Arc<ShaderSpec>> {
        self.stages.get(ShaderStage::Vertex)
    }
}

impl MaterialDef {
    pub fn desc(&self) -> &MaterialDesc {
        &self.desc
    }

    fn vertex_layout(&self) -> &VertexInputLayout {
        &self.desc.vertex_layout
    }

    fn image_bindings(&self) -> &MaterialImageBindings {
        &self.desc.image_bindings
    }

    fn stages(&self) -> &ShaderStageMap {
        &self.desc.stages
    }

    fn cull_mode(&self) -> CullMode {
        self.desc.cull_mode
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
