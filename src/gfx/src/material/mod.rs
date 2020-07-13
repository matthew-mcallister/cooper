use std::sync::Arc;

use base::PartialEnumMap;
use enum_map::{Enum, EnumMap};

use crate::{Globals, SystemState};
use crate::device::{
    DescriptorSet, Image, ImageDef, ImageSubresources, ImageView,
    ImageViewFlags, Sampler, ShaderStageMap,
};

mod simple;
mod state;
mod system;

use simple::*;
crate use state::*;
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
    pub image: Arc<ImageDef>,
    pub flags: ImageViewFlags,
    pub subresources: ImageSubresources,
    pub sampler: Arc<Sampler>,
}

pub type MaterialImageBindings =
    PartialEnumMap<MaterialImage, ImageBindingDesc>;

// TODO: Bake VkPipeline object(s) at material creation time.
#[derive(Debug)]
pub struct MaterialDef {
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
    crate fn values() -> impl ExactSizeIterator<Item = Self> {
        (0..<Self as Enum<()>>::POSSIBLE_VALUES)
            .map(<Self as Enum<()>>::from_usize)
    }
}
