mod trivial;
mod world_render;

crate use trivial::*;
crate use world_render::*;

unit::collect_tests![
    world_render,
];
