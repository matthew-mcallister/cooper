// TODO: asynchronous transfer (for computing mipmaps I guess?)
use std::error::Error;
use std::ffi::c_void;
use std::io;
use std::path::Path;
use std::ptr;
use std::sync::Arc;

use crate::*;

fn begin_one_time() -> vk::CommandBufferBeginInfo {
    vk::CommandBufferBeginInfo {
        flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT_BIT,
        ..Default::default()
    }
}

#[derive(Debug)]
struct StagingBuffer {
    dt: Arc<vkl::DeviceTable>,
    buffer: vk::Buffer,
    ptr: *mut c_void,
    sub_size: usize,
    counter: u64,
}

impl StagingBuffer {
    const SUB_BUFFER_COUNT: usize = 2;

    unsafe fn new(res: &mut InitResources, size: usize) -> Self {
        let objs = &mut res.objs;

        assert_eq!(size % 2, 0);
        let sub_size = size / 2;

        let create_info = vk::BufferCreateInfo {
            size: size as _,
            usage: vk::BufferUsageFlags::TRANSFER_SRC_BIT,
            ..Default::default()
        };
        let buffer = objs.create_buffer(&create_info);
        let alloc = res.mapped_mem.alloc_buffer_memory(buffer);

        StagingBuffer {
            dt: Arc::clone(&objs.device.table),
            buffer,
            ptr: alloc.info().ptr,
            sub_size,
            counter: 0,
        }
    }

    #[inline(always)]
    fn index(&self) -> usize {
        (self.counter % 2) as _
    }

    #[inline(always)]
    fn base_offset(&self) -> usize {
        self.index() * self.sub_size
    }

    #[inline(always)]
    fn sub_buffer(&self) -> *mut [u8] {
        let offset = self.base_offset();
        unsafe {
            let ptr = self.ptr.add(offset) as *mut u8;
            std::slice::from_raw_parts_mut(ptr, self.sub_size) as _
        }
    }

    fn swap(&mut self) {
        self.counter += 1;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandBufferState {
    Initial,
    Recording,
    Executable,
}

impl Default for CommandBufferState {
    fn default() -> Self {
        CommandBufferState::Initial
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct SubBufState {
    xfer_fence: vk::Fence,
    // secondary; vkCmdCopyImage
    copy_cmds: vk::CommandBuffer,
    // primary; vkCmdPipelineBarrier + vkCmdCopyImage
    xfer_cmds: vk::CommandBuffer,
}

#[derive(Debug)]
pub struct ImageUpload {
    dt: Arc<vkl::DeviceTable>,
    gfx_queue: vk::Queue,
    staging: StagingBuffer,
    sub_state: [SubBufState; 2],
    buf: *mut [u8],
    offset: usize,
    pre_barriers: Vec<vk::ImageMemoryBarrier>,
    post_barriers: Vec<vk::ImageMemoryBarrier>,
    rec_state: CommandBufferState,
}

const STAGING_BUFFER_SIZE: usize = 0x100_0000;

impl ImageUpload {
    pub unsafe fn new(res: &mut InitResources, gfx_queue: (u32, vk::Queue)) ->
        Self
    {
        let staging = StagingBuffer::new(res, STAGING_BUFFER_SIZE);

        let objs = &mut res.objs;
        let dt = Arc::clone(&objs.device.table);

        let create_info = vk::CommandPoolCreateInfo {
            flags: vk::CommandPoolCreateFlags::TRANSIENT_BIT
                | vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER_BIT,
            queue_family_index: gfx_queue.0,
            ..Default::default()
        };
        let cmd_pool = objs.create_command_pool(&create_info);

        let l1_alloc_info = vk::CommandBufferAllocateInfo {
            command_pool: cmd_pool,
            command_buffer_count: 2,
            ..Default::default()
        };
        let mut l1_cmds = [vk::CommandBuffer::default(); 2];
        objs.alloc_command_buffers(&l1_alloc_info, &mut l1_cmds[..]);

        let l2_alloc_info = vk::CommandBufferAllocateInfo {
            level: vk::CommandBufferLevel::SECONDARY,
            ..l1_alloc_info
        };
        let mut l2_cmds = [vk::CommandBuffer::default(); 2];
        objs.alloc_command_buffers(&l2_alloc_info, &mut l2_cmds[..]);

        let mut sub_state = [SubBufState::default(); 2];
        for (state, (&l1, &l2)) in sub_state.iter_mut()
            .zip(l1_cmds.iter().zip(l2_cmds.iter()))
        {
            state.xfer_cmds = l1;
            state.copy_cmds = l2;
            state.xfer_fence = objs.create_fence(true);
        }

        let buf = staging.sub_buffer();
        ImageUpload {
            dt,
            gfx_queue: gfx_queue.1,
            staging,
            sub_state,
            buf,
            offset: 0,
            pre_barriers: Vec::new(),
            post_barriers: Vec::new(),
            rec_state: CommandBufferState::Initial,
        }
    }

    #[inline(always)]
    fn state(&self) -> &SubBufState {
        &self.sub_state[self.staging.index()]
    }

    unsafe fn wait_for_xfer(&mut self) {
        let fence = self.state().xfer_fence;
        self.dt.wait_for_fences(1, &fence as _, vk::TRUE, u64::max_value())
            .check_success().unwrap()
    }

    unsafe fn ensure_recording(&mut self) {
        match self.rec_state {
            CommandBufferState::Recording => return,
            CommandBufferState::Executable => unreachable!(),
            _ => {},
        }

        self.rec_state = CommandBufferState::Recording;
        let cmds = self.state().copy_cmds;

        let inheritance_info = Default::default();
        let begin_info = vk::CommandBufferBeginInfo {
            p_inheritance_info: &inheritance_info as _,
            ..begin_one_time()
        };
        self.dt.begin_command_buffer(cmds, &begin_info as _);
    }

    fn reset(&mut self) {
        self.offset = 0;
        self.buf = self.staging.sub_buffer();
        self.pre_barriers.clear();
        self.post_barriers.clear();
        // N.B. The command buffer probably isn't actually in the initial
        // state here since it gets reset implicitly.
        self.rec_state = CommandBufferState::Initial;
    }

    fn reserve(&mut self, size: usize) -> Option<*mut [u8]> {
        assert!(size <= self.staging.sub_size);
        let buf = self.buf;
        let end = self.offset + size;
        unsafe {
            opt(end <= (*buf).len())?;
            let ret = &mut (*buf)[self.offset..end] as _;
            Some(ret)
        }
    }

    fn advance(&mut self, size: usize) {
        assert!(self.offset + size <= self.staging.sub_size);
        self.offset += size
    }

    unsafe fn emit_pre_barrier(
        &mut self,
        image: vk::Image,
        subresource_range: vk::ImageSubresourceRange,
    ) {
        let barrier = vk::ImageMemoryBarrier {
            dst_access_mask: vk::AccessFlags::TRANSFER_WRITE_BIT,
            old_layout: vk::ImageLayout::UNDEFINED,
            new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            image,
            subresource_range,
            ..Default::default()
        };
        self.pre_barriers.push(barrier);
    }

    unsafe fn emit_copy(
        &mut self,
        image: vk::Image,
        regions: &mut [vk::BufferImageCopy],
    ) {
        self.ensure_recording();
        for region in regions.iter_mut() {
            region.buffer_offset += self.offset as u64;
        }
        self.dt.cmd_copy_buffer_to_image(
            self.state().copy_cmds,                 // commandBuffer,
            self.staging.buffer,                    // srcBuffer,
            image,                                  // dstImage,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,  // dstImageLayout,
            regions.len() as _,                     // regionCount,
            regions.as_ptr(),                       // pRegions
        );
    }

    pub unsafe fn emit_post_barrier(&mut self, barrier: vk::ImageMemoryBarrier)
    {
        self.post_barriers.push(barrier);
    }

    unsafe fn record_xfer_cmds(&self) {
        let copy_cmds = self.state().copy_cmds;
        let cmds = self.state().xfer_cmds;
        self.dt.begin_command_buffer(cmds, &begin_one_time() as _);
        self.dt.cmd_pipeline_barrier(
            cmds,                                       // commandBuffer
            vk::PipelineStageFlags::HOST_BIT,           // srcStageMask
            vk::PipelineStageFlags::TRANSFER_BIT,       // dstStageMask
            Default::default(),                 // dependencyFlags
            0,                                  // memoryBarrierCount
            ptr::null(),                        // pMemoryBarriers
            0,                                  // bufferMemoryBarrierCount
            ptr::null(),                        // pBufferMemoryBarriers
            self.pre_barriers.len() as _,       // imageMemoryBarrierCount
            self.pre_barriers.as_ptr(),         // pImageMemoryBarriers
        );
        self.dt.cmd_execute_commands(cmds, 1, &copy_cmds as _);
        self.dt.cmd_pipeline_barrier(
            cmds,                                       // commandBuffer
            vk::PipelineStageFlags::TRANSFER_BIT,       // srcStageMask
            vk::PipelineStageFlags::ALL_GRAPHICS_BIT,   // dstStageMask
            Default::default(),                 // dependencyFlags
            0,                                  // memoryBarrierCount
            ptr::null(),                        // pMemoryBarriers
            0,                                  // bufferMemoryBarrierCount
            ptr::null(),                        // pBufferMemoryBarriers
            self.post_barriers.len() as _,      // imageMemoryBarrierCount
            self.post_barriers.as_ptr(),        // pImageMemoryBarriers
        );
        self.dt.end_command_buffer(cmds).check().unwrap();
    }

    unsafe fn end_recording(&mut self) {
        match self.rec_state {
            CommandBufferState::Initial => {
                assert!(self.pre_barriers.is_empty());
                assert!(self.post_barriers.is_empty());
                return;
            },
            CommandBufferState::Executable => unreachable!(),
            _ => {},
        }
        self.dt.end_command_buffer(self.state().copy_cmds).check().unwrap();
        self.record_xfer_cmds();
        self.rec_state = CommandBufferState::Executable;
    }

    unsafe fn submit(&mut self) {
        match self.rec_state {
            CommandBufferState::Initial => return,
            CommandBufferState::Recording => unreachable!(),
            _ => {},
        }

        let fence = self.state().xfer_fence;
        self.dt.reset_fences(1, &fence as _).check().unwrap();

        let cmds = std::slice::from_ref(&self.state().xfer_cmds);
        let submit_info = vk::SubmitInfo {
            command_buffer_count: cmds.len() as _,
            p_command_buffers: cmds.as_ptr(),
            ..Default::default()
        };
        self.dt.queue_submit(
            self.gfx_queue,
            1,
            &submit_info as _,
            fence,
        ).check().unwrap();
    }

    pub unsafe fn finish_buffer(&mut self) {
        self.end_recording();
        self.submit();
        self.staging.swap();
        self.reset();
        self.wait_for_xfer();
    }

    unsafe fn wait_for_all_xfers(&mut self) {
        let mut fences = [Default::default(); StagingBuffer::SUB_BUFFER_COUNT];
        for (fence, state) in fences.iter_mut().zip(self.sub_state.iter()) {
            *fence = state.xfer_fence;
        }
        self.dt.wait_for_fences(
            fences.len() as _,
            fences.as_ptr(),
            vk::TRUE,
            u64::max_value(),
        ).check_success().unwrap();
    }

    unsafe fn flush(&mut self) {
        self.end_recording();
        self.submit();
        self.staging.swap();
        self.reset();
        self.wait_for_all_xfers();
    }
}

#[derive(Clone, Copy, Debug)]
struct ImageInfo {
    inner: vk::Image,
    view: vk::ImageView,
    alloc: CommonAlloc,
}

#[derive(Debug)]
struct ImageStorage {
    device: Arc<Device>,
    path: Arc<RenderPath>,
    univ_sampler: vk::Sampler,
    image_mem: MemoryPool,
    images: Vec<ImageInfo>,
}

impl ImageStorage {
    unsafe fn new(res: &mut InitResources, path: Arc<RenderPath>) -> Self {
        let device = Arc::clone(&path.swapchain.device);

        let create_info = Default::default();
        let univ_sampler = res.objs.create_sampler(&create_info);

        let type_index = find_memory_type(
            &device,
            vk::MemoryPropertyFlags::DEVICE_LOCAL_BIT,
        ).unwrap();
        let create_info = MemoryPoolCreateInfo {
            type_index,
            mapped: false,
            base_size: 0x100_0000,
        };
        let image_mem = MemoryPool::new(Arc::clone(&device), create_info);

        ImageStorage {
            device,
            path,
            univ_sampler,
            image_mem,
            images: Vec::new(),
        }
    }

    // TODO: Batch writes?
    unsafe fn create_image(
        &mut self,
        create_info: &vk::ImageCreateInfo,
        view_create_info: &mut vk::ImageViewCreateInfo,
    ) -> (u32, &ImageInfo) {
        let dst_set = self.path.texture_set;
        let slot = self.images.len() as _;
        assert!(slot < self.path.max_texture_descriptors);

        let mut image = vk::null();
        self.device.table.create_image
            (create_info as _, ptr::null(), &mut image as _)
            .check().unwrap();

        let alloc = self.image_mem.alloc_image_memory(image);

        view_create_info.image = image;
        let mut view = vk::null();
        self.device.table.create_image_view
            (view_create_info as _, ptr::null(), &mut view as _)
            .check().unwrap();

        self.images.push(ImageInfo { inner: image, view, alloc });

        let image_infos = [vk::DescriptorImageInfo {
            sampler: self.univ_sampler,
            image_view: view,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        }];
        let writes = [vk::WriteDescriptorSet {
            dst_set,
            dst_binding: 0,
            dst_array_element: slot,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            p_image_info: image_infos.as_ptr(),
            ..Default::default()
        }];
        self.device.table.update_descriptor_sets(
            writes.len() as _,
            writes.as_ptr(),
            0,
            ptr::null(),
        );

        (slot, &self.images[slot as usize])
    }
}

impl Drop for ImageStorage {
    fn drop(&mut self) {
        for image in self.images.iter() {
            unsafe {
                self.device.table.destroy_image_view(image.view, ptr::null());
                self.device.table.destroy_image(image.inner, ptr::null());
            }
        }
    }
}

#[derive(Debug)]
pub struct TextureManager {
    device: Arc<Device>,
    path: Arc<RenderPath>,
    gfx_queue: vk::Queue,
    gfx_queue_family: u32,
    storage: ImageStorage,
    upload: ImageUpload,
}

impl TextureManager {
    pub unsafe fn new(
        res: &mut InitResources,
        path: Arc<RenderPath>,
        gfx_queue_family: u32,
    ) -> Self {
        let device = Arc::clone(&path.swapchain.device);
        let gfx_queue = device.get_queue(gfx_queue_family, 0);

        let upload = ImageUpload::new(res, (gfx_queue_family, gfx_queue));
        let storage = ImageStorage::new(res, Arc::clone(&path));

        TextureManager {
            device,
            path,
            gfx_queue,
            gfx_queue_family,
            storage,
            upload,
        }
    }

    unsafe fn reserve(&mut self, size: usize) -> *mut [u8] {
        if let Some(buf) = self.upload.reserve(size) { return buf; }
        self.upload.finish_buffer();
        self.upload.reserve(size).unwrap()
    }

    pub unsafe fn load_image<R: io::Read + io::Seek>(
        &mut self,
        extent: vk::Extent3D,
        format: vk::Format,
        mut stream: R,
    ) -> Result<u32, Box<dyn Error>> {
        let create_info = vk::ImageCreateInfo {
            image_type: vk::ImageType::_2D,
            format,
            extent,
            mip_levels: 1,
            array_layers: 1,
            samples: vk::SampleCountFlags::_1_BIT,
            usage: vk::ImageUsageFlags::TRANSFER_DST_BIT
                | vk::ImageUsageFlags::SAMPLED_BIT,
            initial_layout: vk::ImageLayout::UNDEFINED,
            ..Default::default()
        };
        let subresource_range = vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR_BIT,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        };
        let mut view_create_info = vk::ImageViewCreateInfo {
            view_type: vk::ImageViewType::_2D,
            format,
            subresource_range,
            ..Default::default()
        };
        let (slot, image) =
            self.storage.create_image(&create_info, &mut view_create_info);
        let image = image.inner;

        let size = stream.stream_len()? as usize;
        let stage = &mut *self.reserve(size);
        stream.read_exact(stage)?;

        self.upload.emit_pre_barrier(image, subresource_range);
        self.upload.emit_copy(image, &mut [vk::BufferImageCopy {
            buffer_offset: 0,
            buffer_row_length: extent.width,
            buffer_image_height: extent.height,
            image_subresource: vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR_BIT,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            },
            image_offset: vk::Offset3D::new(0, 0, 0),
            image_extent: extent,
        }]);
        self.upload.emit_post_barrier(vk::ImageMemoryBarrier {
            src_access_mask: vk::AccessFlags::TRANSFER_WRITE_BIT,
            dst_access_mask: vk::AccessFlags::SHADER_READ_BIT,
            old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            image,
            subresource_range,
            ..Default::default()
        });

        self.upload.advance(size);

        Ok(slot)
    }

    pub unsafe fn load_png<P: AsRef<Path>>(&mut self, path: P) ->
        Result<u32, Box<dyn Error>>
    {
        let png = lodepng::decode32_file(path)?;
        let stream = io::Cursor::new(slice_to_bytes(&png.buffer[..]));
        self.load_image(
            (png.width as u32, png.height as u32, 1).into(),
            vk::Format::R8G8B8A8_UNORM,
            stream,
        )
    }

    pub unsafe fn flush(&mut self) {
        self.upload.flush();
    }
}
