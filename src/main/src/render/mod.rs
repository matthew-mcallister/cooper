use std::error::Error;
use std::os::raw::c_char;
use std::ptr;
use std::sync::Arc;

use crate::{glfw, vk, vkl};
use crate::window::Window;

crate mod memory;

const VALIDATION_LAYER: *const c_char =
    c_str!("VK_LAYER_LUNARG_standard_validation");

#[derive(Clone, Debug)]
crate struct VulkanConfig {
    crate enable_validation: bool,
}

// Stores the products of initializing Vulkan
crate struct VulkanSys {
    crate config: VulkanConfig,
    crate _ws: crate::window::System,
    crate entry: vkl::Entry,
    crate inst: vkl::InstanceTable,
    crate pdev: vk::PhysicalDevice,
    crate dev: vkl::DeviceTable,
    crate queue: vk::Queue,
}

impl Drop for VulkanSys {
    fn drop(&mut self) {
        unsafe {
            self.dev.destroy_device(ptr::null());
            self.inst.destroy_instance(ptr::null());
        }
    }
}

fn get_required_device_extensions() -> &'static [*const c_char] {
    &[vk::KHR_SWAPCHAIN_EXTENSION_NAME as *const _ as _]
}

impl VulkanSys {
    crate unsafe fn new(config: VulkanConfig) -> Result<Self, Box<dyn Error>> {
        let _ws = crate::window::System::new()?;

        if glfw::vulkan_supported() != glfw::TRUE {
            Err("Vulkan not supported")?;
        }

        let get_instance_proc_addr = std::mem::transmute({
            glfw::get_instance_proc_address
                (0 as _, c_str!("vkGetInstanceProcAddr"))
        });
        let entry = vkl::Entry::load(get_instance_proc_addr);

        let layers =
            if config.enable_validation { &[VALIDATION_LAYER][..] }
            else { &[][..] };

        let mut num_exts: u32 = 0;
        let exts =
            glfw::get_required_instance_extensions(&mut num_exts as _);

        let app_info = vk::ApplicationInfo {
            s_type: vk::StructureType::APPLICATION_INFO,
            p_next: ptr::null(),
            p_application_name: c_str!("cooper"),
            application_version: vk::make_version!(0, 1, 0),
            p_engine_name: c_str!("cooper"),
            engine_version: vk::make_version!(0, 1, 0),
            api_version: vk::API_VERSION_1_0,
        };
        let create_info = vk::InstanceCreateInfo {
            s_type: vk::StructureType::INSTANCE_CREATE_INFO,
            p_next: ptr::null(),
            flags: Default::default(),
            p_application_info: &app_info as _,
            enabled_layer_count: layers.len() as _,
            pp_enabled_layer_names: layers.as_ptr(),
            enabled_extension_count: num_exts,
            pp_enabled_extension_names: exts,
        };
        let mut inst = vk::null();
        entry.create_instance(&create_info as _, ptr::null(), &mut inst as _)
            .check()?;
        let inst = vkl::InstanceTable::load(inst, get_instance_proc_addr);

        let pdevices = vk::enumerate2!(inst, enumerate_physical_devices)?;
        let pdev = pdevices.into_iter().find(|pd| {
            // NB: future hardware may support presentation on a queue
            // family other than the first and this will no longer work
            glfw::TRUE == glfw::get_physical_device_presentation_support
                (inst.instance.0 as _, pd.0 as _, 0)
        }).ok_or("no presentable graphics device")?;

        let queue_create_info = vk::DeviceQueueCreateInfo {
            s_type: vk::StructureType::DEVICE_QUEUE_CREATE_INFO,
            p_next: ptr::null(),
            flags: Default::default(),
            queue_family_index: 0,
            queue_count: 1,
            p_queue_priorities: &1.0f32 as _,
        };
        let features: vk::PhysicalDeviceFeatures = Default::default();
        let exts = get_required_device_extensions();
        let create_info = vk::DeviceCreateInfo {
            s_type: vk::StructureType::DEVICE_CREATE_INFO,
            p_next: ptr::null(),
            flags: Default::default(),
            queue_create_info_count: 1,
            p_queue_create_infos: &queue_create_info as _,
            enabled_layer_count: 0,
            pp_enabled_layer_names: ptr::null(),
            enabled_extension_count: exts.len() as _,
            pp_enabled_extension_names: exts.as_ptr(),
            p_enabled_features: &features as _,
        };
        let mut dev = vk::null();
        inst.create_device(pdev, &create_info as _, ptr::null(), &mut dev as _)
            .check()?;

        let get_device_proc_addr = std::mem::transmute(get_instance_proc_addr
            (inst.instance, c_str!("vkGetDeviceProcAddr")).unwrap());
        let dev = vkl::DeviceTable::load(dev, get_device_proc_addr);

        let mut queue = vk::null();
        dev.get_device_queue(0, 0, &mut queue as _);

        Ok(VulkanSys {
            config, _ws, entry, inst, pdev, dev, queue,
        })
    }

    crate unsafe fn create_shader_module(&self, src: &[u8]) ->
        Result<vk::ShaderModule, vk::Result>
    {
        assert_eq!(src.len() % 4, 0);
        let create_info = vk::ShaderModuleCreateInfo {
            s_type: vk::StructureType::SHADER_MODULE_CREATE_INFO,
            p_next: ptr::null(),
            flags: Default::default(),
            code_size: src.len() as _,
            p_code: src.as_ptr() as _,
        };
        let mut sm = vk::null();
        self.dev.create_shader_module
            (&create_info, ptr::null(), &mut sm as _).check()?;
        Ok(sm)
    }
}

crate struct VulkanSwapchain {
    crate sys: Arc<VulkanSys>,
    crate win: Arc<Window>,
    crate surface: vk::SurfaceKhr,
    crate swapchain: vk::SwapchainKhr,
    crate create_info: vk::SwapchainCreateInfoKhr,
    crate images: Vec<vk::Image>,
    crate image_views: Vec<vk::ImageView>,
}

impl Drop for VulkanSwapchain {
    fn drop(&mut self) {
        unsafe {
            for &view in self.image_views.iter()
                { self.sys.dev.destroy_image_view(view, ptr::null()); }
            self.sys.dev.destroy_swapchain_khr(self.swapchain, ptr::null());
            self.sys.inst.destroy_surface_khr(self.surface, ptr::null());
        }
    }
}

impl VulkanSwapchain {
    crate unsafe fn new(sys: Arc<VulkanSys>, win: Arc<Window>) ->
        Result<Self, Box<dyn Error>>
    {
        let mut surface: vk::SurfaceKhr = vk::null();
        let res = glfw::create_window_surface(
            sys.inst.instance.0 as _,
            win.inner.as_ptr(),
            0 as *const _,
            &mut surface.0 as _,
        );
        vk::Result(res).check()?;

        let mut result = VulkanSwapchain {
            sys, win, surface, swapchain: vk::null(),
            create_info: Default::default(), images: Vec::new(),
            image_views: Vec::new(),
        };

        // TODO: This design was a faux pas; the surface actually must
        // be created *before* a physical device is chosen.
        let mut supported = Default::default();
        result.sys.inst.get_physical_device_surface_support_khr
            (result.sys.pdev, 0, result.surface, &mut supported as _)
            .check()?;
        if supported == vk::FALSE {
            Err("physical device not supported by surface")?;
            unreachable!();
        }

        result.recreate()?;

        Ok(result)
    }

    crate unsafe fn recreate(&mut self) -> Result<(), Box<dyn Error>> {
        let mut caps: vk::SurfaceCapabilitiesKhr = Default::default();
        self.sys.inst.get_physical_device_surface_capabilities_khr
            (self.sys.pdev, self.surface, &mut caps as _).check()?;

        let min_image_count = if caps.max_image_count > 0 {
            u32::min(caps.min_image_count + 1, caps.max_image_count)
        } else { caps.min_image_count + 1 };

        // The spec says that, on Wayland (and probably other platforms,
        // maybe embedded), the surface extent may be determined by the
        // swapchain extent rather than the other way around.
        if (0xffffffff, 0xffffffff) == caps.current_extent.into()
            { Err("surface extent undefined")?; }

        // TODO: The spec says that you are unable to create a swapchain
        // when this happens. Which platforms do this?
        if (0, 0) == caps.current_extent.into()
            { Err("surface has zero extent")?; }

        let formats = vk::enumerate2!(
            self.sys.inst,
            get_physical_device_surface_formats_khr,
            self.sys.pdev,
            self.surface,
        )?;
        // The first option seems to be best for most common drivers
        let vk::SurfaceFormatKhr { format, color_space } = formats[0];

        let composite_alpha = vk::CompositeAlphaFlagsKhr::OPAQUE_BIT_KHR;
        if !caps.supported_composite_alpha.intersects(composite_alpha)
            { Err("opaque composite alpha mode unavailable")?; }

        let image_usage
            = vk::ImageUsageFlags::COLOR_ATTACHMENT_BIT
            | vk::ImageUsageFlags::TRANSFER_DST_BIT;
        if !caps.supported_usage_flags.contains(image_usage)
            { Err("surface image usage requirements unmet")?; }

        self.create_info = vk::SwapchainCreateInfoKhr {
            s_type: vk::StructureType::SWAPCHAIN_CREATE_INFO_KHR,
            p_next: ptr::null(),
            flags: Default::default(),
            surface: self.surface,
            min_image_count,
            image_format: format,
            image_color_space: color_space,
            image_extent: caps.current_extent,
            image_array_layers: 1,
            image_usage,
            image_sharing_mode: vk::SharingMode::EXCLUSIVE,
            queue_family_index_count: 0,
            p_queue_family_indices: ptr::null(),
            pre_transform: caps.current_transform,
            composite_alpha,
            present_mode: vk::PresentModeKhr::FIFO_KHR,
            clipped: vk::TRUE,
            old_swapchain: self.swapchain,
        };
        self.sys.dev.create_swapchain_khr
            (&self.create_info as _, ptr::null(), &mut self.swapchain as _)
            .check()?;

        self.images = vk::enumerate2!(
            self.sys.dev,
            get_swapchain_images_khr,
            self.swapchain,
        )?;

        for &image in self.images.iter() {
            let create_info = vk::ImageViewCreateInfo {
                s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
                p_next: ptr::null(),
                flags: Default::default(),
                image,
                view_type: vk::ImageViewType::_2D,
                format,
                components: Default::default(),
                subresource_range: vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR_BIT,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                },
            };
            let mut image_view = vk::null();
            self.sys.dev.create_image_view
                (&create_info as _, ptr::null(), &mut image_view as _)
                .check()?;
            self.image_views.push(image_view);
        }

        Ok(())
    }
}

const MEMORY_COUNT: usize = 1;
const DUMMY_IMAGE_BYTES: &[u8] = include_bytes!(asset!("notfound.png"));

const SHADER_VERT_BYTES: &[u8] = include_bytes!(asset!("sprite.vert.spv"));
const SHADER_FRAG_BYTES: &[u8] = include_bytes!(asset!("sprite.frag.spv"));

crate struct Renderer {
    sys: Arc<VulkanSys>,
    swapchain: VulkanSwapchain,
    allocator: memory::DedicatedMemoryAllocator,
    cmd_pool: vk::CommandPool,
    memory: [vk::DeviceMemory; MEMORY_COUNT],
    dummy_image: vk::Image,
    dummy_image_view: vk::ImageView,
    sampler: vk::Sampler,
    set_layout: vk::DescriptorSetLayout,
    layout: vk::PipelineLayout,
    render_pass: vk::RenderPass,
    pipeline: vk::Pipeline,
    framebuffers: Vec<vk::Framebuffer>,
    desc_pool: vk::DescriptorPool,
    desc_set: vk::DescriptorSet,
    draw_cmd_buffers: Vec<vk::CommandBuffer>,
    acquire_semaphore: vk::Semaphore,
    draw_semaphore: vk::Semaphore,
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.sys.dev.device_wait_idle();
            self.sys.dev.destroy_semaphore
                (self.acquire_semaphore, ptr::null());
            self.sys.dev.destroy_semaphore(self.draw_semaphore, ptr::null());
            self.sys.dev.destroy_descriptor_pool(self.desc_pool, ptr::null());
            for &framebuffer in self.framebuffers.iter() {
                self.sys.dev.destroy_framebuffer(framebuffer, ptr::null());
            }
            self.sys.dev.destroy_pipeline(self.pipeline, ptr::null());
            self.sys.dev.destroy_render_pass(self.render_pass, ptr::null());
            self.sys.dev.destroy_pipeline_layout(self.layout, ptr::null());
            self.sys.dev.destroy_descriptor_set_layout
                (self.set_layout, ptr::null());
            self.sys.dev.destroy_sampler(self.sampler, ptr::null());
            self.sys.dev.destroy_image_view
                (self.dummy_image_view, ptr::null());
            self.sys.dev.destroy_image(self.dummy_image, ptr::null());
            for &memory in self.memory.iter()
                { self.sys.dev.free_memory(memory, ptr::null()); }
            self.sys.dev.destroy_command_pool(self.cmd_pool, ptr::null());
        }
    }
}

impl Renderer {
    crate unsafe fn new(swapchain: VulkanSwapchain) ->
        Result<Self, Box<dyn Error>>
    {
        let allocator =
            memory::DedicatedMemoryAllocator::new(Arc::clone(&swapchain.sys));
        let mut out = Renderer {
            sys: Arc::clone(&swapchain.sys),
            swapchain,
            allocator,
            cmd_pool: vk::null(),
            memory: [vk::null(); MEMORY_COUNT],
            dummy_image: vk::null(),
            dummy_image_view: vk::null(),
            sampler: vk::null(),
            set_layout: vk::null(),
            layout: vk::null(),
            render_pass: vk::null(),
            pipeline: vk::null(),
            framebuffers: Vec::new(),
            desc_pool: vk::null(),
            desc_set: vk::null(),
            draw_cmd_buffers: Vec::new(),
            acquire_semaphore: vk::null(),
            draw_semaphore: vk::null(),
        };

        // Create command pool
        let create_info = vk::CommandPoolCreateInfo {
            s_type: vk::StructureType::COMMAND_POOL_CREATE_INFO,
            p_next: ptr::null(),
            flags: Default::default(),
            queue_family_index: 0,
        };
        out.sys.dev.create_command_pool
            (&create_info as _, ptr::null(), &mut out.cmd_pool as _).check()?;

        // Load image
        let png = lodepng::decode32(DUMMY_IMAGE_BYTES).unwrap();
        assert_eq!((png.width, png.height), (64, 64));
        let data = crate::slice_bytes(&png.buffer);
        let format = vk::Format::B8G8R8A8_SRGB;
        let create_info = vk::ImageCreateInfo {
            s_type: vk::StructureType::IMAGE_CREATE_INFO,
            p_next: ptr::null(),
            flags: Default::default(),
            image_type: vk::ImageType::_2D,
            format,
            extent: vk::Extent3D::new(png.width as _, png.height as _, 1),
            mip_levels: 1,
            array_layers: 1,
            samples: vk::SampleCountFlags::_1_BIT,
            tiling: vk::ImageTiling::OPTIMAL,
            usage: vk::ImageUsageFlags::SAMPLED_BIT,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            queue_family_index_count: 0,
            p_queue_family_indices: ptr::null(),
            initial_layout: Default::default(), // ignored
        };
        let (img, mem) = memory::upload_image(&out, &create_info, &data)?;
        out.dummy_image = img;
        out.memory[0] = mem;

        let create_info = vk::ImageViewCreateInfo {
            s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
            p_next: ptr::null(),
            flags: Default::default(),
            image: out.dummy_image,
            view_type: vk::ImageViewType::_2D,
            format,
            components: Default::default(),
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR_BIT,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            },
        };
        out.sys.dev.create_image_view
            (&create_info as _, ptr::null(), &mut out.dummy_image_view as _)
            .check()?;

        // Create pipeline
        out.create_pipeline()?;

        // Create framebuffers
        let swapchain_extent = out.swapchain.create_info.image_extent;
        for image_view in out.swapchain.image_views.iter() {
            let create_info = vk::FramebufferCreateInfo {
                s_type: vk::StructureType::FRAMEBUFFER_CREATE_INFO,
                p_next: ptr::null(),
                flags: Default::default(),
                render_pass: out.render_pass,
                attachment_count: 1,
                p_attachments: image_view as _,
                width: swapchain_extent.width,
                height: swapchain_extent.height,
                layers: 1,
            };
            let mut framebuffer = vk::null();
            out.sys.dev.create_framebuffer
                (&create_info as _, ptr::null(), &mut framebuffer as _)
                .check()?;
            out.framebuffers.push(framebuffer);
        }

        // Create descriptor set
        let pool_size = vk::DescriptorPoolSize {
            type_: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1,
        };
        let create_info = vk::DescriptorPoolCreateInfo {
            s_type: vk::StructureType::DESCRIPTOR_POOL_CREATE_INFO,
            p_next: ptr::null(),
            flags: Default::default(),
            max_sets: 1,
            pool_size_count: 1,
            p_pool_sizes: &pool_size as _,
        };
        out.sys.dev.create_descriptor_pool
            (&create_info as _, ptr::null(), &mut out.desc_pool as _)
            .check()?;

        let alloc_info = vk::DescriptorSetAllocateInfo {
            s_type: vk::StructureType::DESCRIPTOR_SET_ALLOCATE_INFO,
            p_next: ptr::null(),
            descriptor_pool: out.desc_pool,
            descriptor_set_count: 1,
            p_set_layouts: &out.set_layout as _,
        };
        out.sys.dev.allocate_descriptor_sets
            (&alloc_info as _, &mut out.desc_set as _).check()?;

        let image_info = vk::DescriptorImageInfo {
            sampler: vk::null(),
            image_view: out.dummy_image_view,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        };
        let write = vk::WriteDescriptorSet {
            s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
            p_next: ptr::null(),
            dst_set: out.desc_set,
            dst_binding: 0,
            dst_array_element: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            p_image_info: &image_info as _,
            p_buffer_info: ptr::null(),
            p_texel_buffer_view: ptr::null(),
        };
        out.sys.dev.update_descriptor_sets(1, &write as _, 0, ptr::null());

        // Record command buffers
        for &framebuffer in out.framebuffers.iter() {
            let cmd_buffer = out.allocate_command_buffer()?;
            let begin_info = vk::CommandBufferBeginInfo {
                s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
                flags: vk::CommandBufferUsageFlags::SIMULTANEOUS_USE_BIT,
                ..Default::default()
            };
            out.sys.dev.begin_command_buffer(cmd_buffer, &begin_info as _)
                .check()?;

            let clear_value = vk::ClearValue {
                color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 1.0] },
            };
            let begin_info = vk::RenderPassBeginInfo {
                s_type: vk::StructureType::RENDER_PASS_BEGIN_INFO,
                p_next: ptr::null(),
                render_pass: out.render_pass,
                framebuffer,
                render_area: vk::Rect2D::new
                    (Default::default(), swapchain_extent),
                clear_value_count: 1,
                p_clear_values: &clear_value as _,
            };
            out.sys.dev.cmd_begin_render_pass
                (cmd_buffer, &begin_info as _, vk::SubpassContents::INLINE);

            out.sys.dev.cmd_bind_pipeline
                (cmd_buffer, vk::PipelineBindPoint::GRAPHICS, out.pipeline);
            out.sys.dev.cmd_bind_descriptor_sets(
                cmd_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                out.layout,
                0,
                1,
                &out.desc_set as _,
                0,
                ptr::null(),
            );
            out.sys.dev.cmd_draw(cmd_buffer, 6, 1, 0, 0);

            out.sys.dev.cmd_end_render_pass(cmd_buffer);
            out.sys.dev.end_command_buffer(cmd_buffer).check()?;

            out.draw_cmd_buffers.push(cmd_buffer);
        }

        let create_info = vk::SemaphoreCreateInfo {
            s_type: vk::StructureType::SEMAPHORE_CREATE_INFO,
            ..Default::default()
        };
        out.sys.dev.create_semaphore
            (&create_info as _, ptr::null(), &mut out.acquire_semaphore as _)
            .check()?;
        out.sys.dev.create_semaphore
            (&create_info as _, ptr::null(), &mut out.draw_semaphore as _)
            .check()?;

        Ok(out)
    }

    // Split off from the rest of the `new` function due to length.
    unsafe fn create_pipeline(&mut self) -> Result<(), vk::Result> {
        let (mut vert_mod, mut frag_mod) = (vk::null(), vk::null());
        let res: Result<(), vk::Result> = try {
            vert_mod = self.sys.create_shader_module(SHADER_VERT_BYTES)?;
            frag_mod = self.sys.create_shader_module(SHADER_FRAG_BYTES)?;

            // Descriptors and layout
            let create_info = vk::SamplerCreateInfo {
                s_type: vk::StructureType::SAMPLER_CREATE_INFO,
                ..Default::default()
            };
            self.sys.dev.create_sampler
                (&create_info as _, ptr::null(), &mut self.sampler as _)
                .check()?;

            let binding = vk::DescriptorSetLayoutBinding {
                binding: 0,
                descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::FRAGMENT_BIT,
                p_immutable_samplers: &self.sampler as _,
            };
            let create_info = vk::DescriptorSetLayoutCreateInfo {
                s_type: vk::StructureType::DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
                p_next: ptr::null(),
                flags: Default::default(),
                binding_count: 1,
                p_bindings: &binding as _,
            };
            self.sys.dev.create_descriptor_set_layout
                (&create_info as _, ptr::null(), &mut self.set_layout as _)
                .check()?;

            let create_info = vk::PipelineLayoutCreateInfo {
                s_type: vk::StructureType::PIPELINE_LAYOUT_CREATE_INFO,
                set_layout_count: 1,
                p_set_layouts: &self.set_layout as _,
                ..Default::default()
            };
            self.sys.dev.create_pipeline_layout
                (&create_info as _, ptr::null(), &mut self.layout as _)
                .check()?;

            // Render pass
            let color_attachment = vk::AttachmentDescription {
                flags: Default::default(),
                format: self.swapchain.create_info.image_format,
                samples: vk::SampleCountFlags::_1_BIT,
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::STORE,
                stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
                stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
            };
            let color_attachment_ref = vk::AttachmentReference {
                attachment: 0,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            };
            let subpass = vk::SubpassDescription {
                pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
                color_attachment_count: 1,
                p_color_attachments: &color_attachment_ref as _,
                ..Default::default()
            };
            // Dependency on swapchain image acquisition
            let dependency = vk::SubpassDependency {
                src_subpass: vk::SUBPASS_EXTERNAL,
                dst_subpass: 0,
                src_stage_mask:
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT_BIT,
                dst_stage_mask:
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT_BIT,
                src_access_mask: vk::AccessFlags::empty(),
                dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE_BIT,
                dependency_flags: Default::default(),
            };
            let create_info = vk::RenderPassCreateInfo {
                s_type: vk::StructureType::RENDER_PASS_CREATE_INFO,
                p_next: ptr::null(),
                flags: Default::default(),
                attachment_count: 1,
                p_attachments: &color_attachment as _,
                subpass_count: 1,
                p_subpasses: &subpass as _,
                dependency_count: 1,
                p_dependencies: &dependency as _,
            };
            self.sys.dev.create_render_pass
                (&create_info as _, ptr::null(), &mut self.render_pass as _)
                .check()?;

            // Fixed functions
            let shader_stages = [
                vk::PipelineShaderStageCreateInfo {
                    s_type:
                        vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: Default::default(),
                    stage: vk::ShaderStageFlags::VERTEX_BIT,
                    module: vert_mod,
                    p_name: c_str!("main"),
                    p_specialization_info: ptr::null(),
                },
                vk::PipelineShaderStageCreateInfo {
                    s_type:
                        vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: Default::default(),
                    stage: vk::ShaderStageFlags::FRAGMENT_BIT,
                    module: frag_mod,
                    p_name: c_str!("main"),
                    p_specialization_info: ptr::null(),
                },
            ];
            let vertex_input = vk::PipelineVertexInputStateCreateInfo {
                s_type:
                    vk::StructureType::PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO,
                ..Default::default()
            };
            let input_assembly = vk::PipelineInputAssemblyStateCreateInfo {
                s_type: vk::StructureType::
                    PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO,
                topology: vk::PrimitiveTopology::TRIANGLE_LIST,
                ..Default::default()
            };
            let extent = self.swapchain.create_info.image_extent;
            let viewport = vk::Viewport {
                x: 0.0, y: 0.0,
                width: extent.width as _, height: extent.height as _,
                min_depth: 0.0, max_depth: 1.0,
            };
            let scissors = vk::Rect2D::new(Default::default(), extent);
            let viewport_state = vk::PipelineViewportStateCreateInfo {
                s_type: vk::StructureType::PIPELINE_VIEWPORT_STATE_CREATE_INFO,
                viewport_count: 1,
                p_viewports: &viewport as _,
                scissor_count: 1,
                p_scissors: &scissors as _,
                ..Default::default()
            };
            let rasterization = vk::PipelineRasterizationStateCreateInfo {
                s_type: vk::StructureType::
                    PIPELINE_RASTERIZATION_STATE_CREATE_INFO,
                polygon_mode: vk::PolygonMode::FILL,
                cull_mode: vk::CullModeFlags::BACK_BIT,
                front_face: vk::FrontFace::COUNTER_CLOCKWISE,
                line_width: 1.0,
                ..Default::default()
            };
            let multisample = vk::PipelineMultisampleStateCreateInfo {
                s_type:
                    vk::StructureType::PIPELINE_MULTISAMPLE_STATE_CREATE_INFO,
                rasterization_samples: vk::SampleCountFlags::_1_BIT,
                ..Default::default()
            };
            let alpha_blend = vk::PipelineColorBlendAttachmentState {
                blend_enable: vk::TRUE,
                src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
                dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
                color_blend_op: vk::BlendOp::ADD,
                src_alpha_blend_factor: vk::BlendFactor::ONE,
                dst_alpha_blend_factor: vk::BlendFactor::ZERO,
                alpha_blend_op: vk::BlendOp::ADD,
                color_write_mask: vk::ColorComponentFlags::R_BIT
                    | vk::ColorComponentFlags::G_BIT
                    | vk::ColorComponentFlags::B_BIT
                    | vk::ColorComponentFlags::A_BIT,
            };
            let color_blend = vk::PipelineColorBlendStateCreateInfo {
                s_type:
                    vk::StructureType::PIPELINE_COLOR_BLEND_STATE_CREATE_INFO,
                attachment_count: 1,
                p_attachments: &alpha_blend as _,
                ..Default::default()
            };

            // Final creation
            let create_info = vk::GraphicsPipelineCreateInfo {
                s_type: vk::StructureType::GRAPHICS_PIPELINE_CREATE_INFO,
                p_next: ptr::null(),
                flags: Default::default(),
                stage_count: 2,
                p_stages: &shader_stages as _,
                p_vertex_input_state: &vertex_input as _,
                p_input_assembly_state: &input_assembly as _,
                p_tessellation_state: ptr::null(),
                p_viewport_state: &viewport_state as _,
                p_rasterization_state: &rasterization as _,
                p_multisample_state: &multisample as _,
                p_depth_stencil_state: ptr::null(),
                p_color_blend_state: &color_blend as _,
                p_dynamic_state: ptr::null(),
                layout: self.layout,
                render_pass: self.render_pass,
                subpass: 0,
                base_pipeline_handle: vk::null(),
                base_pipeline_index: -1,
            };
            self.sys.dev.create_graphics_pipelines(
                vk::null(),
                1,
                &create_info as _,
                ptr::null(),
                &mut self.pipeline as _,
            );
        };
        self.sys.dev.destroy_shader_module(frag_mod, ptr::null());
        self.sys.dev.destroy_shader_module(vert_mod, ptr::null());
        res
    }

    crate unsafe fn allocate_command_buffer(&self) ->
        Result<vk::CommandBuffer, vk::Result>
    {
        let alloc_info = vk::CommandBufferAllocateInfo {
            s_type: vk::StructureType::COMMAND_BUFFER_ALLOCATE_INFO,
            p_next: ptr::null(),
            command_pool: self.cmd_pool,
            level: vk::CommandBufferLevel::PRIMARY,
            command_buffer_count: 1,
        };
        let mut cmd_buf = vk::null();
        self.sys.dev.allocate_command_buffers(&alloc_info, &mut cmd_buf as _)
            .check()?;
        Ok(cmd_buf)
    }

    crate unsafe fn do_frame(&self) -> Result<(), vk::Result> {
        let mut idx: u32 = 0;
        self.sys.dev.acquire_next_image_khr(
            self.swapchain.swapchain,
            !0,
            self.acquire_semaphore,
            vk::null(),
            &mut idx as _,
        ).check()?;

        let wait_stages = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT_BIT;
        let submit_info = vk::SubmitInfo {
            s_type: vk::StructureType::SUBMIT_INFO,
            p_next: ptr::null(),
            wait_semaphore_count: 1,
            p_wait_semaphores: &self.acquire_semaphore as _,
            p_wait_dst_stage_mask: &wait_stages as _,
            command_buffer_count: 1,
            p_command_buffers: &self.draw_cmd_buffers[idx as usize],
            signal_semaphore_count: 1,
            p_signal_semaphores: &self.draw_semaphore as _,
        };
        self.sys.dev.queue_submit
            (self.sys.queue, 1, &submit_info as _, vk::null());

        let present_info = vk::PresentInfoKhr {
            s_type: vk::StructureType::PRESENT_INFO_KHR,
            p_next: ptr::null(),
            wait_semaphore_count: 1,
            p_wait_semaphores: &self.draw_semaphore as _,
            swapchain_count: 1,
            p_swapchains: &self.swapchain.swapchain as _,
            p_image_indices: &idx as _,
            p_results: ptr::null_mut(),
        };
        self.sys.dev.queue_present_khr(self.sys.queue, &present_info as _)
            .check()?;
        Ok(())
    }
}
