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
    pub buffer: vk::Buffer,
    pub offset: vk::DeviceSize,
    pub range: vk::DeviceSize,
    pub data: *mut [Sprite],
}

impl SpriteBuffer {
    pub unsafe fn new_pair(
        res: &mut InitResources,
        size: u32,
    ) -> [Self; 2] {
        let objs = &mut res.objs;
        let mapped_mem = &mut res.mapped_mem;
        assert!(mapped_mem.mapped());

        let buf_size = size as vk::DeviceSize *
            std::mem::size_of::<Sprite>() as vk::DeviceSize;
        let create_info = vk::BufferCreateInfo {
            size: 2 * buf_size,
            usage: vk::BufferUsageFlags::STORAGE_BUFFER_BIT,
            ..Default::default()
        };
        let buffer = objs.create_buffer(&create_info);
        let mem = mapped_mem.alloc_buffer_memory(buffer);

        let size = size as usize;
        let slice: *mut [Sprite] = mem.info().as_slice();
        [
            SpriteBuffer {
                buffer,
                offset: 0,
                range: buf_size,
                data: &mut (*slice)[..size] as _,
            },
            SpriteBuffer {
                buffer,
                offset: buf_size,
                range: buf_size,
                data: &mut (*slice)[size..] as _,
            },
        ]
    }
}
