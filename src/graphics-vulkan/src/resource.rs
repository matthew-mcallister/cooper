use std::marker::PhantomData;
use std::ptr;
use std::sync::Arc;

use crate::*;

#[derive(Clone, Copy, Debug)]
crate struct Image {
    crate view: vk::ImageView,
    crate image: vk::Image,
    crate memory: CommonAlloc,
}

impl Resource for Image {
    unsafe fn destroy(self, res: &mut ResourceStorage) {
        res.dt.destroy_image_view(self.view, ptr::null());
        res.dt.destroy_image(self.image, ptr::null());
        res.image_alloc.free(&self.memory);
    }
}

#[derive(Clone, Copy, Debug)]
crate struct Buffer {
    crate buffer: vk::Buffer,
    crate memory: CommonAlloc,
}

impl Resource for Buffer {
    unsafe fn destroy(self, res: &mut ResourceStorage) {
        res.dt.destroy_buffer(self.buffer, ptr::null());
        res.buffer_alloc.free(&self.memory);
    }
}

#[derive(Debug)]
crate struct ResourceStorage {
    dt: Arc<vkl::DeviceTable>,
    buffer_alloc: MemoryPool,
    image_alloc: MemoryPool,
    samplers: Arena<Sampler>,
    images: Arena<Image>,
    materials: Arena<Material>,
    geometries: Arena<Geometry>,
}

crate trait HasResource<T> {
    type Id;

    fn table(&self) -> &Arena<T>;

    fn table_mut(&mut self) -> &mut Arena<T>;

    fn insert(&mut self, val: T) -> Self::Id {
        self.table_mut().insert(val)
    }

    fn add_ref(&mut self, id: Self::Id) {
        self.table_mut().add_ref(id);
    }

    fn sub_ref(&mut self, id: Self::Id) {
        if let Some(obj) = self.table_mut().sub_ref(id) {
            unsafe { obj.destroy(self); }
        }
    }

    fn get(&self, id: Self::Id) -> &T {
        self.table().get(id)
    }

    fn get_mut(&mut self, id: Self::Id) -> &mut T {
        self.table_mut().get_mut(id)
    }
}

macro_rules! impl_resources {
    ($(($type:ident, $table:ident),)*) => {
        $(
            impl HasResource<$type> for ResourceStorage {
                fn table(&self) -> &Arena<$type> {
                    &self.$table
                }
                fn table_mut(&mut self) -> &mut Arena<$type> {
                    &mut self.$table
                }
            }
        )*
    }
}

impl_resources! {
    (Buffer, buffers),
    (Image, images),
}

impl ResourceStorage {
    pub unsafe fn create_buffer(
        &mut self,
        create_info: &vk::BufferCreateInfo,
    ) -> BufferId {
        let (buffer, memory) = self.buffer_alloc.create_buffer(&create_info);
        self.insert(Buffer { buffer, memory })
    }

    pub unsafe fn create_image(
        &mut self,
        mut create_info: vk::ImageCreateInfo,
        view_info: &vk::ImageViewCreateInfo,
    ) -> ImageId {
        let (image, memory) = self.image_alloc.create_image(&create_info);
        let mut view = vk::null();
        self.dt.create_image_view(&view_info, ptr::null(), &mut view as _);
        self.insert(Image { image, view, memory })
    }
}

const STAGING_BUFFER_COUNT: usize = 2;

/// This utility type is responsible for transferring data from host
/// memory to device memory.
#[derive(Debug)]
pub struct ResourceStaging {
    dt: Arc<vkl::DeviceTable>,
    pool: vk::CommandPool,
    buffers: [StagingBuffer; STAGING_BUFFER_COUNT],
    active: Option<usize>,
    xfer_queue: vk::Queue,
}

#[derive(Debug)]
struct StagingBuffer {
    memory: vk::DeviceMemory,
    buffer: vk::Buffer,
    slice: *mut [u8],
    xfer_cmds: vk::CommandBuffer,
    fence: vk::Fence,
    semaphore: vk::Semaphore,
}

impl ResourceStaging {
    fn staging(&self) -> &StagingBuffer {
        &self.buffers[self.active.unwrap()]
    }

    fn staging_mut(&mut self) -> &mut StagingBuffer {
        &mut self.buffers[self.active.unwrap()]
    }

    /// Executes any pending transfers and image operations.
    ///
    /// This is automatically called by the destructor.
    pub unsafe fn flush(&mut self) {
    }

    pub unsafe fn staging_is_available(&self, index: u32) -> bool {
    }

    pub unsafe fn wait_for_staging(&mut self, index: u32) {
    }

    /// Returns the mapped memory range of the current staging buffer.
    pub fn staging_data(&mut self) -> &mut [u8] {
        let slice = self.staging().slice;
        unsafe { &*slice }
    }

    /// Copies data from the active staging buffer to another buffer.
    pub unsafe fn copy_to_buffer(
        &mut self,
        buffer: vk::Buffer,
        copies: &[vk::BufferCopy],
    ) {
        let stg = self.staging();
        self.dt.cmd_copy_buffer_to_image(
            stg.xfer_cmds,
            stg.buffer,
            buffer,
            copies.len() as _,
            copies.as_ptr(),
        );
        // TODO: queue ownership transfer
    }

    /// Copies data from the active staging buffer to an image
    /// subresource.
    pub unsafe fn copy_to_image(
        &mut self,
        image: vk::Image,
        copies: &[vk::BufferImageCopy],
    ) {
        let stg = self.staging();
        self.dt.cmd_copy_buffer_to_image(
            stg.xfer_cmds,
            stg.buffer,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            copies.len() as _,
            copies.as_ptr(),
        );
    }
}

/// Generate mipmaps on-device. Currently uses a linear filter, which is
/// ideal for load times, but a future downsampling algorithm may give
/// superior results. Run these right after copying the image from
/// staging (and possibly transferring queues).
///
/// # Preconditions
///
/// Any steps taken to fulfill these must happen-before the recorded
/// commands.
///
/// - The value of `mip_levels` is greater than 1 for the image.
/// - All image subresources (except possibly the last mip level) must
///   have TRANSFER_SRC_BIT in their usage flags.
/// - The VkFormatProperties for the image format must have the
///   BLIT_SRC_BIT and SAMPLED_IMAGE_FILTER_LINEAR_BIT flags set.
/// - The image format must not require sampler Ycbcr conversion.
/// - All image subresources must be in the TRANSFER_DST_OPTIMAL layout.
/// - All image subresources must be owned by the queue that executes
///   the recorded commands, which must support graphics operations.
///
/// # Postconditions
///
/// All image mip levels will be in the `final_layout` (the intermediate
/// layout(s) used are an implementation detail).
pub fn generate_mipmaps(
    dt: &vkl::DeviceTable,
    cmds: vk::CommandBuffer,
    image: vk::Image,
    create_info: &vk::ImageCreateInfo,
    final_layout: vk::ImageLayout,
) {
    assert!(mip_levels > 1, "mip_levels: {}", mip_levels);
    assert_ne!(array_layers, 0);
    for m in 1..mip_levels {
        let barrier = vk::ImageMemoryBarrier {
            src_access_mask: vk::AccessFlags::TRANSFER_WRITE_BIT,
            dst_access_mask: vk::AccessFlags::TRANSFER_READ_BIT,
            old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            new_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            image,
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspect::COLOR_BIT,
                base_mip_level: m - 1,
                level_count: 1,
                base_array_layer: 0,
                layer_count: array_layers,
            },
            ..Default::default()
        };
        self.dt.cmd_pipeline_barrier(
            cmds,
            vk::PipelineStageFlags::TRANSFER_BIT,
            vk::PipelineStageFlags::TRANSFER_BIT,
            Default::default(),
            0, ptr::null(),
            0, ptr::null(),
            1, &barrier as _,
        );

        let mk_subresource = |mip_level| vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspect::COLOR_BIT,
            mip_level,
            base_array_layer: 0,
            layer_count: array_layers,
        };
        let mk_offsets = |n| [
            vk::Offset3D::new(0, 0, 0),
            vk::Offset3D {
                x: extent.width as u32 >> n,
                y: extent.height as u32 >> n,
                z: extent.depth as u32 >> n,
            },
        ];
        let blit = vk::ImageBlit {
            src_subresource: mk_subresource(m - 1),
            src_offsets: mk_offsets(m - 1),
            dst_subresource: mk_subresource(m),
            src_offsets: mk_offsets(m),
        };
        self.dt.cmd_blit_image(
            cmds,
            image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            image, vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            region_count: 1,
            p_regions: &blit as _,
            vk::Filter::LINEAR,
        );
    }
}
