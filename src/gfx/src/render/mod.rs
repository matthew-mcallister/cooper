use crate::*;

mod debug;
mod view;
mod world_render;

pub use debug::*;
pub use view::*;
crate use world_render::*;

crate trait Renderer {
    fn compile_material(
        &self,
        program: MaterialProgram,
        images: &MaterialImageMap,
    ) -> Option<DescriptorSet>;
}

#[cfg(test)]
mod trivial;
#[cfg(test)]
crate use trivial::*;

unit::collect_tests![
    world_render,
    trivial,
];
