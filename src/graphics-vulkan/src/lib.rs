//! TODO: Consider testing with LunarG device simulator
#![feature(crate_visibility_modifier)]
#![feature(try_blocks)]

macro_rules! c_str {
    ($str:expr) => {
        concat!($str, "\0") as *const str as *const std::os::raw::c_char;
    }
}

macro_rules! insert_nodup {
    ($map:expr, $key:expr, $val:expr) => {
        assert!(!$map.insert($key, $val).is_some());
    }
}

macro_rules! impl_debug_union {
    ($name:ident) => {
        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, concat!(stringify!($name), " {{ *union* }}"))
            }
        }
    }
}

mod descriptor;
mod fixed;
mod geom;
mod init;
mod memory;
mod pipeline;
mod resource;

crate use descriptor::*;
crate use fixed::*;
crate use geom::*;
pub use init::*;
crate use memory::*;
crate use pipeline::*;
crate use resource::*;

fn bool32(b: bool) -> vk::Bool32 {
    if b { vk::TRUE } else { vk::FALSE }
}

#[inline]
fn align_to(alignment: vk::DeviceSize, offset: vk::DeviceSize) ->
    vk::DeviceSize
{
    ((offset + alignment - 1) / alignment) * alignment
}

pub unsafe fn do_test(swapchain: &Swapchain) -> Result<(), vk::Result> {
    let samplers = stock_samplers(swapchain);
    let set_layouts = stock_set_layouts(swapchain, &samplers);
    let render_passes = stock_render_passes(swapchain);
    let shaders = stock_shaders(swapchain);
    let pipeline_layouts = stock_pipeline_layouts(swapchain, &set_layouts);
    let _pipelines = create_pipeline_map
        (swapchain, &render_passes, &shaders, &pipeline_layouts);
    Ok(())
}
