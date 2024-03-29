use std::ffi::{c_void, CStr};
use std::fmt;
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use derive_more::*;
use itertools::Itertools;

use crate::*;

/// Adds type information to Vulkan object types from the debug_utils
/// extension.
// TODO: should be part of vk::HandleType
pub trait DebugHandle: vk::traits::HandleType + Copy {
    /// Returns the debug object type.
    fn object_type() -> vk::ObjectType;
}

pub trait Named: fmt::Debug {
    /// Returns the debugging name of the object.
    fn name(&self) -> Option<&str>;
}

pub fn write_named<N: Named>(named: &N, f: &mut fmt::Formatter) -> fmt::Result {
    if let Some(name) = named.name() {
        // TODO: Would prefer to include the type name
        write!(f, "<{}>", name)
    } else {
        // TODO for structured logging: Generate numeric IDs for unnamed
        // variables that appear in logs, like how LLVM IR uses IDs for
        // unnamed SSA variables. Not at runtime, of course.
        write!(f, "{:?}", named)
    }
}

pub fn fmt_named<'a>(named: &'a impl Named) -> impl fmt::Debug + 'a {
    struct FmtNamed<'a, N: Named>(&'a N);
    impl<N: Named> fmt::Debug for FmtNamed<'_, N> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write_named(self.0, f)
        }
    }
    FmtNamed(named)
}

macro_rules! impl_debug_marker_name {
    ($($type:ident = $value:ident;)*) => {
        $(
            impl DebugHandle for vk::$type {
                fn object_type() -> vk::ObjectType {
                    vk::ObjectType::$value
                }
            }
        )*

        #[derive(Debug)]
        struct ObjectType(vk::ObjectType);

        impl fmt::Display for ObjectType {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{}", match self.0 {
                    $(vk::ObjectType::$value => stringify!($type),)*
                    _ => "unknown type",
                })
            }
        }
    }
}

// TODO: Ideally this would be generated from the registry
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
    DebugUtilsMessengerEXT = DEBUG_UTILS_MESSENGER_EXT;
    ValidationCacheEXT = VALIDATION_CACHE_EXT;
    AccelerationStructureNV = ACCELERATION_STRUCTURE_NV;
}

pub unsafe fn set_name<T: DebugHandle>(
    device: &vkl::DeviceTable,
    handle: T,
    name: &impl AsRef<std::ffi::CStr>,
) {
    let info = vk::DebugUtilsObjectNameInfoEXT {
        object_type: T::object_type(),
        object_handle: handle.into(),
        p_object_name: name.as_ref().as_ptr(),
        ..Default::default()
    };
    device.set_debug_utils_object_name_ext(&info);
}

pub(crate) trait DebugMessageHandler: fmt::Debug + Send + Sync {
    fn handle(
        &self,
        severity: vk::DebugUtilsMessageSeverityFlagBitsEXT,
        types: vk::DebugUtilsMessageTypeFlagsEXT,
        data: &vk::DebugUtilsMessengerCallbackDataEXT,
    );
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct DebugMessenger {
    inner: vk::DebugUtilsMessengerEXT,
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    types: vk::DebugUtilsMessageTypeFlagsEXT,
    handler: Box<Arc<dyn DebugMessageHandler>>,
}

impl DebugMessenger {
    // TODO: This is an *instance-level* method...
    pub unsafe fn new(
        instance: &Instance,
        severity: vk::DebugUtilsMessageSeverityFlagsEXT,
        types: vk::DebugUtilsMessageTypeFlagsEXT,
        handler: Arc<dyn DebugMessageHandler>,
    ) -> Self {
        let it = &*instance.table;
        let handler = Box::new(handler);
        let create_info = vk::DebugUtilsMessengerCreateInfoEXT {
            message_severity: severity,
            message_type: types,
            pfn_user_callback: Some(debug_message_handler as _),
            p_user_data: &*handler as *const Arc<_> as _,
            ..Default::default()
        };
        let mut inner = vk::null();
        it.create_debug_utils_messenger_ext(&create_info, ptr::null(), &mut inner)
            .check()
            .unwrap();
        Self {
            inner,
            severity,
            types,
            handler,
        }
    }

    pub unsafe fn destroy(&mut self, it: &vkl::InstanceTable) {
        it.destroy_debug_utils_messenger_ext(self.inner, ptr::null());
    }
}

unsafe extern "C" fn debug_message_handler(
    message_severity: vk::DebugUtilsMessageSeverityFlagBitsEXT,
    message_types: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    p_user_data: *mut c_void,
) -> vk::Bool32 {
    let handler: *const Arc<dyn DebugMessageHandler> = p_user_data as _;
    (*handler).handle(message_severity, message_types, &*p_callback_data);
    vk::FALSE
}

#[derive(Debug, Display)]
#[display(fmt = "{}", name)]
pub(crate) struct Label {
    pub(crate) name: String,
    pub(crate) color: [f32; 4],
}

impl Label {
    unsafe fn from_vk(label: &vk::DebugUtilsLabelEXT) -> Self {
        let name = CStr::from_ptr(label.p_label_name)
            .to_str()
            .unwrap()
            .to_owned();
        Self {
            name,
            color: label.color,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ObjectInfo {
    pub(crate) ty: vk::ObjectType,
    pub(crate) handle: u64,
    pub(crate) name: Option<String>,
}

impl ObjectInfo {
    unsafe fn from_vk(info: &vk::DebugUtilsObjectNameInfoEXT) -> Self {
        let name = info.p_object_name;
        let name = if !name.is_null() {
            Some(CStr::from_ptr(name).to_str().unwrap().to_owned())
        } else {
            None
        };
        Self {
            ty: info.object_type,
            handle: info.object_handle,
            name,
        }
    }

    fn name(&self) -> Option<&str> {
        Some(&self.name.as_ref()?)
    }
}

impl fmt::Display for ObjectInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "{} 0x{:016x}[{}]",
            ObjectType(self.ty),
            self.handle,
            self.name().unwrap_or(""),
        )
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct DebugMessagePayload {
    pub(crate) message_severity: vk::DebugUtilsMessageSeverityFlagBitsEXT,
    pub(crate) message_types: vk::DebugUtilsMessageTypeFlagsEXT,
    pub(crate) message_id_name: String,
    pub(crate) message_id: i32,
    pub(crate) message: String,
    pub(crate) queue_labels: Vec<Label>,
    pub(crate) cmd_buf_labels: Vec<Label>,
    pub(crate) objects: Vec<ObjectInfo>,
}

impl DebugMessagePayload {
    unsafe fn from_vk(
        message_severity: vk::DebugUtilsMessageSeverityFlagBitsEXT,
        message_types: vk::DebugUtilsMessageTypeFlagsEXT,
        data: &vk::DebugUtilsMessengerCallbackDataEXT,
    ) -> Self {
        let message_id_name = CStr::from_ptr(data.p_message_id_name)
            .to_str()
            .unwrap()
            .to_owned();
        let message = CStr::from_ptr(data.p_message).to_str().unwrap().to_owned();
        let queue_labels =
            std::slice::from_raw_parts(data.p_queue_labels, data.queue_label_count as _)
                .iter()
                .map(|x| Label::from_vk(x))
                .collect();
        let cmd_buf_labels =
            std::slice::from_raw_parts(data.p_cmd_buf_labels, data.cmd_buf_label_count as _)
                .iter()
                .map(|x| Label::from_vk(x))
                .collect();
        let objects = std::slice::from_raw_parts(data.p_objects, data.object_count as _)
            .iter()
            .map(|x| ObjectInfo::from_vk(x))
            .collect();
        Self {
            message_severity,
            message_types,
            message_id_name,
            message_id: data.message_id_number,
            message,
            queue_labels,
            cmd_buf_labels,
            objects,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Severity(vk::DebugUtilsMessageSeverityFlagsEXT);

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use vk::DebugUtilsMessageSeverityFlagsEXT as Flags;
        write!(
            f,
            "{}",
            match self.0 {
                Flags::VERBOSE_BIT_EXT => "VERBOSE",
                Flags::INFO_BIT_EXT => "INFO",
                Flags::WARNING_BIT_EXT => "WARNING",
                Flags::ERROR_BIT_EXT => "ERROR",
                _ => "unknown severity",
            }
        )
    }
}

#[derive(Clone, Copy, Debug)]
struct Type(vk::DebugUtilsMessageTypeFlagsEXT);

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use vk::DebugUtilsMessageTypeFlagBitsEXT as Flags;

        let ty = self.0;
        let pairs = [
            (Flags::GENERAL_BIT_EXT, "GENERAL"),
            (Flags::VALIDATION_BIT_EXT, "VALIDATION"),
            (Flags::PERFORMANCE_BIT_EXT, "PERFORMANCE"),
        ];

        if !pairs.iter().any(|&(k, _)| ty.contains(k)) {
            return write!(f, "unknown type");
        }

        let fmt = pairs
            .iter()
            .filter_map(|&(k, v)| ty.contains(k).then_some(v))
            .format(" | ");
        write!(f, "{}", fmt)
    }
}

impl fmt::Display for DebugMessagePayload {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "[{}][{}] {}: {}",
            Severity(self.message_severity),
            Type(self.message_types),
            self.message_id_name,
            self.message,
        )?;

        if !self.objects.is_empty() {
            writeln!(f, "  Objects:")?;
            for obj in self.objects.iter() {
                writeln!(f, "    {}", obj)?;
            }
        }

        fn write_labels(f: &mut fmt::Formatter, prefix: &str, labels: &[Label]) -> fmt::Result {
            if !labels.is_empty() {
                writeln!(f, "  {}:", prefix)?;
            }
            for label in labels.iter() {
                writeln!(f, "    {}", label.name)?;
            }
            Ok(())
        }

        write_labels(f, "Queue labels", &self.queue_labels)?;
        write_labels(f, "Command buffer labels", &self.cmd_buf_labels)?;

        Ok(())
    }
}

#[derive(Debug, Default)]
pub(crate) struct DefaultDebugMessageHandler {
    count: AtomicU32,
}

impl DefaultDebugMessageHandler {
    pub(crate) fn message_count(&self) -> u32 {
        self.count.load(Ordering::Relaxed)
    }
}

impl DebugMessageHandler for DefaultDebugMessageHandler {
    fn handle(
        &self,
        severity: vk::DebugUtilsMessageSeverityFlagBitsEXT,
        types: vk::DebugUtilsMessageTypeFlagsEXT,
        data: &vk::DebugUtilsMessengerCallbackDataEXT,
    ) {
        let payload = unsafe { DebugMessagePayload::from_vk(severity, types, data) };
        eprintln!("{}", payload);
        self.count.fetch_add(1, Ordering::Relaxed);
    }
}
