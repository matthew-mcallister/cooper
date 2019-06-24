use std::ptr;

use crate::*;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SpriteTransform {
    /// A row-contiguous matrix
    pub mat: [[f32; 2]; 2],
    pub offset: [f32; 2],
}

/// A sprite is a textured quad with lighting and other effects applied.
/// The quad for a sprite is given by applying a transformation to the
/// unit square in clip space.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sprite {
    pub transform: SpriteTransform,
    pub textures: [u32; 2],
}

#[derive(Debug)]
pub struct SpriteBuffer {
    pub data: *mut [Sprite],
    pub desc_set: vk::DescriptorSet,
}

impl SpriteBuffer {
    pub unsafe fn new_pair(
        path: &RenderPath,
        objs: &mut ObjectTracker,
        mapped_mem: &mut MemoryPool,
        size: u32,
    ) -> [Self; 2] {
        let dt = &path.swapchain.device.table;
        assert!(mapped_mem.mapped());

        let buf_size = size as vk::DeviceSize *
            std::mem::size_of::<Sprite>() as vk::DeviceSize;
        let create_info = vk::BufferCreateInfo {
            size: 2 * buf_size,
            usage: vk::BufferUsageFlags::STORAGE_BUFFER_BIT,
            ..Default::default()
        };
        let (buffer, mem) = mapped_mem.alloc_buffer(&create_info);
        objs.buffers.push(buffer);

        let set_layout = &path.sprite_set_layout;
        let params = CreateDescriptorSetParams {
            count: 2,
            ..Default::default()
        };
        let (_, sets) = create_descriptor_sets(objs, set_layout, params);

        let offsets = [0, buf_size];
        for (&set, &offset) in sets.iter().zip(offsets.iter()) {
            let buf_writes = [vk::DescriptorBufferInfo {
                buffer,
                offset,
                range: buf_size,
            }];
            let writes = [vk::WriteDescriptorSet {
                dst_set: set,
                dst_binding: 0,
                descriptor_count: 1,
                descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
                p_buffer_info: buf_writes.as_ptr(),
                ..Default::default()
            }];
            dt.update_descriptor_sets(
                writes.len() as _,
                writes.as_ptr(),
                0,
                ptr::null(),
            );
        }

        let size = size as usize;
        let slice: *mut [Sprite] = mem.info().as_slice();
        [
            SpriteBuffer {
                data: &mut (*slice)[..size] as _,
                desc_set: sets[0],
            },
            SpriteBuffer {
                data: &mut (*slice)[size..] as _,
                desc_set: sets[1],
            },
        ]
    }
}
