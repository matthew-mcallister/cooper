use std::os::raw::c_char;

/// Supplies useful information for using debug extensions.
pub trait CanDebug: vk::traits::HandleType {
    /// Returns the debug object type.
    fn object_type() -> vk::ObjectType;
}

macro_rules! impl_debug_marker_name {
    ($($type:ident = $value:ident;)*) => {
        $(
            impl CanDebug for vk::$type {
                fn object_type() -> vk::ObjectType {
                    vk::ObjectType::$value
                }
            }
        )*
    }
}

impl_debug_marker_name! {
    Instance = INSTANCE;
    PhysicalDevice = PHYSICAL_DEVICE;
    Device = DEVICE;
    Queue = QUEUE;
    Semaphore = SEMAPHORE;
    CommandBuffer = COMMAND_BUFFER;
    Fence = FENCE;
    DeviceMemory = DEVICE_MEMORY;
    Buffer = BUFFER;
    Image = IMAGE;
    Event = EVENT;
    QueryPool = QUERY_POOL;
    BufferView = BUFFER_VIEW;
    ImageView = IMAGE_VIEW;
    ShaderModule = SHADER_MODULE;
    PipelineCache = PIPELINE_CACHE;
    PipelineLayout = PIPELINE_LAYOUT;
    RenderPass = RENDER_PASS;
    Pipeline = PIPELINE;
    DescriptorSetLayout = DESCRIPTOR_SET_LAYOUT;
    Sampler = SAMPLER;
    DescriptorPool = DESCRIPTOR_POOL;
    DescriptorSet = DESCRIPTOR_SET;
    Framebuffer = FRAMEBUFFER;
    CommandPool = COMMAND_POOL;
    SamplerYcbcrConversion = SAMPLER_YCBCR_CONVERSION;
    DescriptorUpdateTemplate = DESCRIPTOR_UPDATE_TEMPLATE;
    SurfaceKHR = SURFACE_KHR;
    SwapchainKHR = SWAPCHAIN_KHR;
    DisplayKHR = DISPLAY_KHR;
    DisplayModeKHR = DISPLAY_MODE_KHR;
    DebugReportCallbackEXT = DEBUG_REPORT_CALLBACK_EXT;
    ObjectTableNVX = OBJECT_TABLE_NVX;
    IndirectCommandsLayoutNVX = INDIRECT_COMMANDS_LAYOUT_NVX;
    DebugUtilsMessengerEXT = DEBUG_UTILS_MESSENGER_EXT;
    ValidationCacheEXT = VALIDATION_CACHE_EXT;
    AccelerationStructureNV = ACCELERATION_STRUCTURE_NV;
}

pub unsafe fn set_debug_name<T: CanDebug>(
    device: &vkl::DeviceTable,
    object: T,
    name: *const c_char,
) {
    let info = vk::DebugUtilsObjectNameInfoEXT {
        object_type: T::object_type(),
        object_handle: object.into(),
        p_object_name: name,
        ..Default::default()
    };
    device.set_debug_utils_object_name_ext(&info as _);
}
