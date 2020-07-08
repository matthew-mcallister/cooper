mod manager;
mod scheduler;
mod staging;
mod state;

crate use manager::*;
use scheduler::*;
use staging::*;
crate use state::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceState {
    Available,
    Pending,
    Unavailable,
}

unit::collect_tests![
    manager,
    staging,
];
