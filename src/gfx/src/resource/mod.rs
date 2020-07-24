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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
crate enum SchedulerStatus {
    Busy,
    Idle,
}

unit::collect_tests![
    staging,
    system,
];
