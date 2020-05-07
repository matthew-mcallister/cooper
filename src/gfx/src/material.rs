use std::sync::Arc;

use enum_map::{Enum, EnumMap};

use crate::*;

/// An identifier of a particular material rendering technique.
// TODO: Should be serializable to/from a string.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub enum MaterialProgram {
    Debug(DebugDisplay),
}

#[derive(Clone, Copy, Debug, Hash, Enum, Eq, PartialEq)]
pub enum MaterialImage {
    Albedo,
    Normal,
    MetallicRoughness,
    // etc.
}

pub type MaterialImageMap = EnumMap<MaterialImage, Option<Arc<ImageView>>>;

// TODO?: nodes
#[derive(Debug)]
pub struct Material {
    crate program: MaterialProgram,
    crate images: MaterialImageMap,
    // Some material types
    crate desc: Option<DescriptorSet>,
}
