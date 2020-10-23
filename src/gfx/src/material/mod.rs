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

// TODO: This type (a) doesn't actually represent a physical material
// and (b) is tightly coupled to the choice of geometry. I think it only
// makes sense to join it with the mesh to create some kind of "render
// atom" which is the smallest unit which can be meaningfully rendered.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct MaterialDesc {
    pub vertex_layout: VertexInputLayout,
    pub stages: ShaderStageMap,
    /// Binds image handles to material image slots. Slots without
    /// explicit bindings will have a default image/sampler pair bound
    /// to them.
    // TODO: User should have to opt-in to using the default bindings.
    // Or, better yet, provide defaults on their own.
    pub image_bindings: MaterialImageBindings,
    pub cull_mode: CullMode,
}

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
