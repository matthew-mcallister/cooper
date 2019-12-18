use std::sync::Arc;

use crate::*;

#[derive(Debug)]
crate struct ImageInfo {
    crate inner: vk::Image,
    crate extent: vk::Extent3D,
    crate format: vk::Format,
    crate samples: vk::SampleCountFlags,
    crate layers: u32,
    crate mip_levels: u32,
}

crate unsafe fn create_image_view(device: &Device, info: &ImageInfo) ->
    vk::ImageView
{
    let dt = &*device.table;
    let create_info = vk::ImageViewCreateInfo {
        image: info.inner,
        view_type: vk::ImageViewType::_2D,
        format: info.format,
        subresource_range: vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR_BIT,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        },
        ..Default::default()
    };
    let mut view = vk::null();
    dt.create_image_view(&create_info, ptr::null(), &mut view)
        .check().unwrap();
    view
}
