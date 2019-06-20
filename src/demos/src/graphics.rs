use std::ptr;
use std::sync::Arc;

use crate::{Device, Swapchain, Timestamps};

#[derive(Debug)]
pub struct GfxObjects {
    pub device: Arc<Device>,
    pub command_pools: Vec<vk::CommandPool>,
    pub pipelines: Vec<vk::Pipeline>,
    pub shader_modules: Vec<vk::ShaderModule>,
    pub pipeline_layouts: Vec<vk::PipelineLayout>,
    pub framebuffers: Vec<vk::Framebuffer>,
    pub render_passes: Vec<vk::RenderPass>,
    pub image_views: Vec<vk::ImageView>,
    pub semaphores: Vec<vk::Semaphore>,
    pub fences: Vec<vk::Fence>,
    pub query_pools: Vec<vk::QueryPool>,
}

macro_rules! impl_drop {
    ($(($field:ident, $destructor:ident),)*) => {
        impl Drop for GfxObjects {
            fn drop(&mut self) {
                unsafe {
                    self.device.table.device_wait_idle();
                    $(
                        for &obj in self.$field.iter() {
                            self.device.table.$destructor(obj, ptr::null());
                        }
                    )*
                }
            }
        }
    }
}

impl_drop! {
    (command_pools, destroy_command_pool),
    (pipelines, destroy_pipeline),
    (shader_modules, destroy_shader_module),
    (pipeline_layouts, destroy_pipeline_layout),
    (framebuffers, destroy_framebuffer),
    (render_passes, destroy_render_pass),
    (image_views, destroy_image_view),
    (semaphores, destroy_semaphore),
    (fences, destroy_fence),
    (query_pools, destroy_query_pool),
}

impl GfxObjects {
    pub fn new(device: &Arc<Device>) -> Box<Self> {
        let res = GfxObjects {
            device: Arc::clone(device),
            command_pools: Vec::new(),
            pipelines: Vec::new(),
            shader_modules: Vec::new(),
            pipeline_layouts: Vec::new(),
            framebuffers: Vec::new(),
            render_passes: Vec::new(),
            image_views: Vec::new(),
            semaphores: Vec::new(),
            fences: Vec::new(),
            query_pools: Vec::new(),
        };
        Box::new(res)
    }

    pub unsafe fn create_command_pool(
        &mut self,
        create_info: &vk::CommandPoolCreateInfo,
    ) -> vk::CommandPool {
        let mut obj = vk::null();
        self.device.table.create_command_pool
            (create_info as _, ptr::null(), &mut obj as _)
            .check().unwrap();
        self.command_pools.push(obj);
        obj
    }

    pub unsafe fn alloc_command_buffers(
        &mut self,
        alloc_info: &vk::CommandBufferAllocateInfo,
        res: &mut [vk::CommandBuffer],
    ) {
        assert_eq!(alloc_info.command_buffer_count as usize, res.len());
        self.device.table.allocate_command_buffers
            (alloc_info as _, res.as_mut_ptr())
            .check().unwrap();
    }

    pub unsafe fn create_shader(&mut self, code: &[u8]) -> vk::ShaderModule {
        assert_eq!(code.len() % 4, 0);
        let create_info = vk::ShaderModuleCreateInfo {
            code_size: code.len(),
            p_code: code.as_ptr() as _,
            ..Default::default()
        };

        let mut obj = vk::null();
        self.device.table.create_shader_module
            (&create_info as _, ptr::null(), &mut obj as _)
            .check().unwrap();
        self.shader_modules.push(obj);
        obj
    }

    pub unsafe fn create_render_pass(
        &mut self,
        create_info: &vk::RenderPassCreateInfo,
    ) -> vk::RenderPass {
        let mut obj = vk::null();
        self.device.table.create_render_pass
            (create_info as _, ptr::null(), &mut obj as _)
            .check().unwrap();
        self.render_passes.push(obj);
        obj
    }

    pub unsafe fn create_framebuffer(
        &mut self,
        create_info: &vk::FramebufferCreateInfo,
    ) -> vk::Framebuffer {
        let mut obj = vk::null();
        self.device.table.create_framebuffer
            (create_info as _, ptr::null(), &mut obj as _)
            .check().unwrap();
        self.framebuffers.push(obj);
        obj
    }

    pub unsafe fn create_pipeline_layout(
        &mut self,
        create_info: &vk::PipelineLayoutCreateInfo,
    ) -> vk::PipelineLayout {
        let mut obj = vk::null();
        self.device.table.create_pipeline_layout
            (create_info as _, ptr::null(), &mut obj as _)
            .check().unwrap();
        self.pipeline_layouts.push(obj);
        obj
    }

    pub unsafe fn create_graphics_pipeline(
        &mut self,
        create_info: &vk::GraphicsPipelineCreateInfo,
    ) -> vk::Pipeline {
        let mut obj = vk::null();
        self.device.table.create_graphics_pipelines(
            vk::null(),
            1,
            create_info as _,
            ptr::null(),
            &mut obj as _,
        ).check().unwrap();
        self.pipelines.push(obj);
        obj
    }

    pub unsafe fn create_image_view(
        &mut self,
        create_info: &vk::ImageViewCreateInfo,
    ) -> vk::ImageView {
        let mut obj = vk::null();
        self.device.table.create_image_view
            (create_info as _, ptr::null(), &mut obj as _)
            .check().unwrap();
        self.image_views.push(obj);
        obj
    }

    pub unsafe fn create_fence(&mut self, signaled: bool) -> vk::Fence {
        let flags = if signaled { vk::FenceCreateFlags::SIGNALED_BIT }
            else { Default::default() };
        let create_info = vk::FenceCreateInfo {
            flags,
            ..Default::default()
        };
        let mut obj = vk::null();
        self.device.table.create_fence
            (&create_info as _, ptr::null(), &mut obj as _)
            .check().unwrap();
        self.fences.push(obj);
        obj
    }

    pub unsafe fn create_semaphore(&mut self) -> vk::Semaphore {
        let create_info = Default::default();
        let mut obj = vk::null();
        self.device.table.create_semaphore
            (&create_info as _, ptr::null(), &mut obj as _)
            .check().unwrap();
        self.semaphores.push(obj);
        obj
    }

    pub unsafe fn create_query_pool(
        &mut self,
        create_info: &vk::QueryPoolCreateInfo,
    ) -> vk::QueryPool {
        let mut obj = vk::null();
        self.device.table.create_query_pool
            (create_info as _, ptr::null(), &mut obj as _)
            .check().unwrap();
        self.query_pools.push(obj);
        obj
    }
}

pub unsafe fn create_swapchain_image_view(
    gfx: &mut GfxObjects,
    swapchain: &Swapchain,
    image: vk::Image,
) -> vk::ImageView {
    let create_info = vk::ImageViewCreateInfo {
        image,
        view_type: vk::ImageViewType::_2D,
        format: swapchain.format,
        subresource_range: vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR_BIT,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        },
        ..Default::default()
    };
    gfx.create_image_view(&create_info as _)
}

pub unsafe fn create_swapchain_framebuffer(
    gfx: &mut GfxObjects,
    swapchain: &Swapchain,
    render_pass: vk::RenderPass,
    view: vk::ImageView,
) -> vk::Framebuffer {
    let attachments = std::slice::from_ref(&view);
    let create_info = vk::FramebufferCreateInfo {
        render_pass,
        attachment_count: attachments.len() as _,
        p_attachments: attachments.as_ptr(),
        width: swapchain.extent.width,
        height: swapchain.extent.height,
        layers: 1,
        ..Default::default()
    };
    gfx.create_framebuffer(&create_info as _)
}

#[repr(C)]
#[derive(Debug)]
pub struct DeviceTimer {
    pub device: Arc<Device>,
    pub query_pool: vk::QueryPool,
}

impl DeviceTimer {
    pub unsafe fn new(gfx: &mut GfxObjects) -> Self {
        let create_info = vk::QueryPoolCreateInfo {
            query_type: vk::QueryType::TIMESTAMP,
            query_count: 2,
            ..Default::default()
        };
        let query_pool = gfx.create_query_pool(&create_info);
        DeviceTimer {
            device: Arc::clone(&gfx.device),
            query_pool,
        }
    }

    pub unsafe fn get_query_results(&self) -> Result<Timestamps, vk::Result> {
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
        ).check_success()?;
        Ok(ts)
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
