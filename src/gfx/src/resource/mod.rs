mod scheduler;
mod staging;
mod state;
mod system;

use scheduler::*;
use staging::*;
crate use state::*;
crate use system::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceState {
    Available,
    Pending,
    Unavailable,
}

unit::collect_tests![
    staging,
    system,
];
