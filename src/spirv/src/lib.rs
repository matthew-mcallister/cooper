#![feature(crate_visibility_modifier, trait_alias)]

use spirv_headers as spv;

mod build;
mod data;
mod view;

pub use build::{parse_bytes, parse_words};
pub use data::Module;
pub use view::*;

pub use spv::ExecutionModel;
pub use spv::StorageClass;

crate fn is_interface_storage(class: spv::StorageClass) -> bool {
    [spv::StorageClass::Input, spv::StorageClass::Output].contains(&class)
}
