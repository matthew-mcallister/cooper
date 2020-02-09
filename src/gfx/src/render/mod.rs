mod trivial;
mod world;

crate use trivial::*;
crate use world::*;

unit::collect_tests![
    world,
];
