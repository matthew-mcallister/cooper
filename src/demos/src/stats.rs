use std::sync::Arc;

use crate::*;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Timestamps {
    pub old: u64,
    pub new: u64,
}

impl Timestamps {
    pub fn to_ns(self, device: &Device) -> f32 {
        let timestamp_period = device.props.limits.timestamp_period;
        ((self.new - self.old) as f64 * timestamp_period as f64) as f32
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct FrameTimer {
    pub device: Arc<Device>,
    pub query_pool: vk::QueryPool,
}

impl FrameTimer {
    pub unsafe fn new(objs: &mut ObjectTracker) -> Self {
        let create_info = vk::QueryPoolCreateInfo {
            query_type: vk::QueryType::TIMESTAMP,
            query_count: 2,
            ..Default::default()
        };
        let query_pool = objs.create_query_pool(&create_info);
        FrameTimer {
            device: Arc::clone(&objs.device),
            query_pool,
        }
    }

    pub unsafe fn get_query_results(&self) -> Timestamps {
        let mut ts: Timestamps = Default::default();
        let data_size = std::mem::size_of::<Timestamps>();
        let stride = std::mem::size_of::<u64>();
        self.device.table.get_query_pool_results(
            self.query_pool,                // queryPool
            0,                              // firstQuery
            2,                              // queryCount
            data_size,                      // dataSize
            &mut ts as *mut _ as _,         // pData
            stride as _,                    // stride
            vk::QueryResultFlags::_64_BIT,  // flags
        ).check_success().unwrap();
        ts
    }

    pub unsafe fn start(&self, cb: vk::CommandBuffer) {
        self.device.table.cmd_reset_query_pool(cb, self.query_pool, 0, 2);
        self.device.table.cmd_write_timestamp(
            cb,
            vk::PipelineStageFlags::TOP_OF_PIPE_BIT,
            self.query_pool,
            0,
        );
    }

    pub unsafe fn end(&self, cb: vk::CommandBuffer) {
        self.device.table.cmd_write_timestamp(
            cb,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE_BIT,
            self.query_pool,
            1,
        );
    }
}
