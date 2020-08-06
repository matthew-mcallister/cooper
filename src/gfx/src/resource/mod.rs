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

#[derive(Debug)]
pub struct BufferDef {
    pub binding: device::BufferBinding,
    pub lifetime: device::Lifetime,
    pub mapping: device::MemoryMapping,
    pub size: vk::DeviceSize,
}

unit::collect_tests![
    staging,
    system,
];
