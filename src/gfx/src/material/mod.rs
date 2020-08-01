use std::sync::Arc;

use base::PartialEnumMap;
use device::{
    DescriptorSet, Image, ImageDef, ImageSubresources, ImageView,
    ImageViewFlags, Sampler, ShaderStageMap,
};
use enum_map::{Enum, EnumMap};

use crate::SystemState;

mod geom;
mod state;
mod system;
mod texture;

use geom::*;
crate use state::*;
crate use system::*;
use texture::*;

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

#[derive(Clone, Debug)]
pub struct ImageBindingDesc {
    // TODO: Maybe should create an "ImageViewDesc" type.
    pub image: Arc<ImageDef>,
    // TODO: Should be set by the factory, not the user
    pub flags: ImageViewFlags,
    pub subresources: ImageSubresources,
    pub sampler: Arc<Sampler>,
}

pub type MaterialImageBindings =
    PartialEnumMap<MaterialImage, ImageBindingDesc>;

// TODO: Bake VkPipeline object(s) at material creation time.
#[derive(Debug)]
pub struct MaterialDef {
    // TODO: Shouldn't own this pointer (may hold on to internal
    // resources).
    factory: Arc<dyn MaterialFactory>,
    program: MaterialProgram,
    image_bindings: MaterialImageBindings,
}

#[derive(Clone, Debug)]
crate struct ImageBindingState {
    crate view: Arc<ImageView>,
    crate sampler: Arc<Sampler>,
}

crate type MaterialImageState =
    PartialEnumMap<MaterialImage, ImageBindingState>;

/// A loaded material with all shader bindings available.
#[derive(Debug)]
crate struct Material {
    def: Arc<MaterialDef>,
    images: MaterialImageState,
    desc: Option<DescriptorSet>,
}

impl MaterialDef {
    pub fn program(&self) -> MaterialProgram {
        self.program
    }

    pub fn image_bindings(&self) -> &MaterialImageBindings {
        &self.image_bindings
    }
}

#[allow(dead_code)]
impl Material {
    crate fn def(&self) -> &Arc<MaterialDef> {
        &self.def
    }

    crate fn images(&self) -> &MaterialImageState {
        &self.images
    }

    crate fn desc(&self) -> Option<&DescriptorSet> {
        self.desc.as_ref()
    }

    crate fn select_shaders(&self) -> ShaderStageMap {
        self.def.factory.select_shaders()
    }
}

impl MaterialImage {
    const SIZE: usize = <Self as Enum<()>>::POSSIBLE_VALUES;

    crate fn values() -> impl ExactSizeIterator<Item = Self> {
        (0..Self::SIZE).map(<Self as Enum<()>>::from_usize)
    }
}
