mod instance;
mod scheduler;
mod view;
mod world_render;

pub use instance::*;
crate use scheduler::*;
pub use view::*;
crate use world_render::*;

#[cfg(test)]
mod trivial;
#[cfg(test)]
crate use trivial::*;

unit::collect_tests![
    scheduler,
    trivial,
];
