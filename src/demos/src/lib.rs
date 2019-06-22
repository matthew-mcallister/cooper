#![feature(try_blocks)]

#[macro_export]
macro_rules! c_str {
    ($str:expr) => {
        concat!($str, "\0") as *const str as *const std::os::raw::c_char
    }
}

#[macro_export]
macro_rules! include_shader {
    ($name:expr) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            concat!("/generated/shaders/", $name),
        ))
    }
}

mod init;
mod object;

pub use init::*;
pub use object::*;

pub unsafe fn create_swapchain_image_view(
    objs: &mut ObjectTracker,
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
    objs.create_image_view(&create_info as _)
}

pub unsafe fn create_swapchain_framebuffer(
    objs: &mut ObjectTracker,
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
    objs.create_framebuffer(&create_info as _)
}
