use std::mem::MaybeUninit;
use std::sync::Arc;

use derive_more::Display;

use crate::*;

#[derive(Clone, Copy, Debug, Default, Display, Eq, PartialEq)]
#[display(fmt = "staging buffer out of memory")]
crate struct StagingOutOfMemory;
impl std::error::Error for StagingOutOfMemory {}

/// Staging type for uploading images and buffers
// TODO: Need a scheduler, preferably one that makes it easy to specify
// complex transfer operations and makes efficient use of the buffer.
#[derive(Debug)]
crate struct XferStage {
    staging: StagingBuffer,
    pre_barriers: Vec<vk::ImageMemoryBarrier>,
    post_barriers: Vec<vk::ImageMemoryBarrier>,
    image_copies: Vec<ImageCopy>,
}

#[derive(Debug)]
struct ImageCopy {
    // TODO: Shouldn't use a reference count here.
    // Ed: The solution to this is garbage collection. Then you can just
    // mark an image as "live" and be done with it.
    image: Arc<Image>,
    region: vk::BufferImageCopy,
}

impl XferStage {
    crate fn new(device: Arc<Device>, capacity: usize) -> Self {
        XferStage {
            staging: StagingBuffer::new(device, capacity),
            pre_barriers: Vec::new(),
            post_barriers: Vec::new(),
            image_copies: Vec::new(),
        }
    }

    crate unsafe fn stage_buffer(&self) {
        // No need to stage buffers on UMA
        // TODO: assert!(!buffer.is_device_local());
        todo!()
    }

    /// In the returned buffer, mipmap levels are allocated contiguously
    /// starting from the base mipmap level.
    crate fn stage_image(
        &mut self,
        image: &Arc<Image>,
        emit_pre_barrier: bool,
        final_layout: vk::ImageLayout,
        access_mask: vk::AccessFlags,
        subresources: ImageSubresources,
    ) -> Result<&mut [u8], StagingOutOfMemory> {
        let sub = subresources;
        image.validate_subresources(&sub);
        assert_eq!(sub.aspects, image.format().aspects());
        assert_eq!(image.samples(), SampleCount::One);

        let size = image.subresource_size(&sub) as usize;
        let mut alloc = self.staging.alloc(size).ok_or(StagingOutOfMemory)?;

        let extent = image.extent();
        for mip_level in subresources.mip_level_range() {
            let mip_extent = extent.mip_level(mip_level);
            self.image_copies.push(ImageCopy {
                // TODO: Duplicated ref counts? Yuck.
                image: Arc::clone(image),
                region: vk::BufferImageCopy {
                    buffer_offset: alloc.offset,
                    image_subresource: subresources.to_mip_layers(mip_level),
                    image_extent: mip_extent.into(),
                    ..Default::default()
                },
            });
        }

        if emit_pre_barrier {
            self.pre_barriers.push(vk::ImageMemoryBarrier {
                src_access_mask: Default::default(),
                dst_access_mask: vk::AccessFlags::TRANSFER_WRITE_BIT,
                old_layout: vk::ImageLayout::UNDEFINED,
                new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                image: image.inner(),
                subresource_range: subresources.into(),
                ..Default::default()
            });
        }
        self.post_barriers.push(vk::ImageMemoryBarrier {
            src_access_mask: vk::AccessFlags::TRANSFER_WRITE_BIT,
            dst_access_mask: access_mask,
            old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            new_layout: final_layout,
            image: image.inner(),
            subresource_range: subresources.into(),
            ..Default::default()
        });

        unsafe {
            let slice = alloc.as_mut_slice::<u8>(size);
            Ok(&mut *(MaybeUninit::slice_get_mut(slice) as *mut _))
        }
    }

    crate unsafe fn record_cmds(&self, _cmds: &mut XferCmds) {
        todo!()
    }

    crate fn clear(&mut self) {
        self.staging.clear();
        self.pre_barriers.clear();
        self.post_barriers.clear();
        self.image_copies.clear();
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use super::*;

    unsafe fn staging_inner(heap: &DeviceHeap, staging: &mut XferStage) {
        let extent = Extent3D::new(128, 128, 1);
        let img = Arc::new(Image::new(
            &heap,
            Default::default(),
            ImageType::Dim2,
            Format::RGBA8,
            SampleCount::One,
            extent,
            extent.mip_levels(),
            6,
        ));

        let subresource = ImageSubresources {
            aspects: img.format().aspects(),
            mip_levels: [0, extent.mip_levels()],
            layers: [0, 6],
        };
        let buf = staging.stage_image(
            &img,
            true,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            vk::AccessFlags::SHADER_READ_BIT,
            subresource,
        ).unwrap();
        assert_eq!(buf.len(), img.subresource_size(&subresource) as usize);

        assert_eq!(staging.pre_barriers.len(), 1);
        assert_eq!(staging.post_barriers.len(), 1);
        assert_eq!(staging.image_copies.len(), extent.mip_levels() as usize);
    }

    unsafe fn stage(vars: testing::TestVars) {
        let device = vars.device();
        let mut staging = XferStage::new(Arc::clone(&device), 0x10_0000);

        let state = SystemState::new(Arc::clone(&device));

        // Run test, clear, and run it again
        staging_inner(&state.heap, &mut staging);
        staging.clear();
        staging_inner(&state.heap, &mut staging);
    }

    unit::declare_tests![stage];
}

unit::collect_tests![tests];
