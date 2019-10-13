use std::f32::consts::PI;
use std::ffi::CString;
use std::marker::PhantomData;
use std::ops::Range;
use std::ptr;
use std::sync::Arc;

use cooper_graphics_vulkan::*;
use math::*;
use prelude::*;

#[macro_use]
mod common;

use common::*;

unsafe fn init_resources(
    swapchain: Arc<Swapchain>,
    queues: Vec<Vec<Arc<Queue>>>,
) -> AppResources {
    let window = Arc::clone(&swapchain.surface.window);
    let device = Arc::clone(&swapchain.device);

    let mut set_layouts = DescriptorSetLayoutManager::new(Arc::clone(&device));
    let bindings = [
        vk::DescriptorSetLayoutBinding {
            binding: 0,
            descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::VERTEX_BIT
                | vk::ShaderStageFlags::FRAGMENT_BIT,
            p_immutable_samplers: ptr::null(),
        },
        vk::DescriptorSetLayoutBinding {
            binding: 1,
            descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::VERTEX_BIT
                | vk::ShaderStageFlags::FRAGMENT_BIT,
            p_immutable_samplers: ptr::null(),
        },
    ];
    let create_args = DescriptorSetLayoutCreateArgs {
        bindings: &bindings[..],
        ..Default::default()
    };
    let set_layout_name = "globals".to_owned();
    set_layouts.create_layout(set_layout_name.clone(), &create_args);
    let set_layouts = Arc::new(set_layouts);

    let mut pipe_layouts =
        PipelineLayoutManager::new(Arc::clone(&set_layouts));
    pipe_layouts.create_layout(
        set_layout_name.clone(),
        vec![set_layout_name.clone()],
    );
    let pipe_layouts = Arc::new(pipe_layouts);

    let mut shaders = ShaderManager::new(Arc::clone(&device));
    shaders.create_shader("example_vert".to_owned(), ShaderDesc {
        entry: CString::new("main".to_owned()).unwrap(),
        code: include_shader!("example_vert.spv").to_vec(),
        set_bindings: Vec::new(),
    });
    shaders.create_shader("example_frag".to_owned(), ShaderDesc {
        entry: CString::new("main".to_owned()).unwrap(),
        code: include_shader!("example_frag.spv").to_vec(),
        set_bindings: Vec::new(),
    });
    let shaders = Arc::new(shaders);

    let mut render_passes = RenderPassManager::new(Arc::clone(&device));
    let attachments = [vk::AttachmentDescription {
        format: swapchain.format,
        samples: vk::SampleCountFlags::_1_BIT,
        load_op: vk::AttachmentLoadOp::CLEAR,
        store_op: vk::AttachmentStoreOp::STORE,
        initial_layout: vk::ImageLayout::UNDEFINED,
        final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
        ..Default::default()
    }];
    let color_attachs = [vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    }];
    let subpasses = [vk::SubpassDescription {
        pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
        color_attachment_count: color_attachs.len() as _,
        p_color_attachments: color_attachs.as_ptr(),
        ..Default::default()
    }];
    let create_info = vk::RenderPassCreateInfo {
        attachment_count: attachments.len() as _,
        p_attachments: attachments.as_ptr(),
        subpass_count: subpasses.len() as _,
        p_subpasses: subpasses.as_ptr(),
        ..Default::default()
    };
    render_passes.create_render_pass(
        "forward".to_owned(),
        &create_info,
        vec!["lighting".to_owned()],
    );
    let render_passes = Arc::new(render_passes);

    let attachments = Arc::new(AttachmentChain::from_swapchain(&swapchain));
    let framebuffers = Arc::new(render_passes.create_framebuffers(
        "forward".to_owned(),
        vec![attachments],
    ));

    AppResources {
        window,
        swapchain,
        queues,
        set_layouts,
        pipe_layouts,
        shaders,
        render_passes,
        framebuffers,
    }
}

type PipelineDesc = ();

#[derive(Debug)]
struct PipelineFactory {
    res: Arc<AppResources>,
}

impl PipelineFactory {
    fn new(res: Arc<AppResources>) -> Self {
        PipelineFactory {
            res,
        }
    }
}

impl GraphicsPipelineFactory for PipelineFactory {
    type Desc = PipelineDesc;

    unsafe fn create_pipeline(&mut self, _: &Self::Desc) -> GraphicsPipeline {
        let swapchain = &self.res.swapchain;
        let dt = &swapchain.device.table;

        let render_passes = &self.res.render_passes;
        let shaders = &self.res.shaders;
        let pipe_layouts = &self.res.pipe_layouts;

        let vert = shaders.get("example_vert");
        let frag = shaders.get("example_frag");

        let vert_stage = vk::PipelineShaderStageCreateInfo {
            stage: vk::ShaderStageFlags::VERTEX_BIT,
            module: vert.inner,
            p_name: vert.entry().as_ptr(),
            ..Default::default()
        };
        let frag_stage = vk::PipelineShaderStageCreateInfo {
            stage: vk::ShaderStageFlags::FRAGMENT_BIT,
            module: frag.inner,
            p_name: frag.entry().as_ptr(),
            ..Default::default()
        };
        let stages = vec![vert_stage, frag_stage];

        let layout_id = "globals";
        let layout = pipe_layouts.get(layout_id).inner;

        let render_pass_id = "forward";
        let render_pass = render_passes.get(render_pass_id);
        let subpass_id = "lighting";
        let subpass = render_pass.subpasses[subpass_id];
        let render_pass = render_pass.inner;

        let (bindings, attrs) = Mesh::bindings_and_attrs();
        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo {
            vertex_binding_description_count: bindings.len() as _,
            p_vertex_binding_descriptions: bindings.as_ptr(),
            vertex_attribute_description_count: attrs.len() as _,
            p_vertex_attribute_descriptions: attrs.as_ptr(),
            ..Default::default()
        };
        let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo {
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            ..Default::default()
        };

        let viewports = [swapchain.viewport()];
        let scissors = [swapchain.rect()];
        let viewport_state = vk::PipelineViewportStateCreateInfo {
            viewport_count: viewports.len() as _,
            p_viewports: viewports.as_ptr(),
            scissor_count: scissors.len() as _,
            p_scissors: scissors.as_ptr(),
            ..Default::default()
        };

        let rasterization_state = vk::PipelineRasterizationStateCreateInfo {
            cull_mode: vk::CullModeFlags::BACK_BIT,
            line_width: 1.0,
            ..Default::default()
        };

        let multisample_state = vk::PipelineMultisampleStateCreateInfo {
            rasterization_samples: vk::SampleCountFlags::_1_BIT,
            ..Default::default()
        };

        let color_blend_atts = [vk::PipelineColorBlendAttachmentState {
            color_write_mask: vk::ColorComponentFlags::R_BIT
                | vk::ColorComponentFlags::G_BIT
                | vk::ColorComponentFlags::B_BIT
                | vk::ColorComponentFlags::A_BIT,
            ..Default::default()
        }];
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo {
            attachment_count: color_blend_atts.len() as _,
            p_attachments: color_blend_atts.as_ptr(),
            ..Default::default()
        };

        let create_info = vk::GraphicsPipelineCreateInfo {
            stage_count: stages.len() as _,
            p_stages: stages.as_ptr(),
            p_vertex_input_state: &vertex_input_state,
            p_input_assembly_state: &input_assembly_state,
            p_viewport_state: &viewport_state,
            p_rasterization_state: &rasterization_state,
            p_multisample_state: &multisample_state,
            p_color_blend_state: &color_blend_state,
            layout,
            render_pass,
            subpass,
            ..Default::default()
        };
        let create_infos = std::slice::from_ref(&create_info);

        let mut inner = vk::null();
        let pipelines = std::slice::from_mut(&mut inner);
        dt.create_graphics_pipelines(
            vk::null(),                 // pipelineCache
            create_infos.len() as _,    // createInfoCount
            create_infos.as_ptr(),      // pCreateInfos
            ptr::null(),                // pAllocator
            pipelines.as_mut_ptr(),     // pPipelines
        ).check().unwrap();

        GraphicsPipeline {
            inner,
            layout: layout_id.to_owned(),
            render_pass: render_pass_id.to_owned(),
            subpass: subpass_id.to_owned(),
        }
    }
}

// Suballocates vertex/index buffers.
#[derive(Debug)]
struct VertexBufferManager {
    mem: MemoryPool,
}

impl VertexBufferManager {
    unsafe fn new(device: Arc<Device>) -> Self {
        // XXX: Mapped memory suboptimal for
        // XXX: This is hardcoded for Intel
        let type_index = 1;
        let create_info = MemoryPoolCreateInfo {
            type_index,
            base_size: 0x100_0000,
            host_mapped: true,
            buffer_map: Some(BufferMapOptions {
                usage: vk::BufferUsageFlags::TRANSFER_DST_BIT
                    | vk::BufferUsageFlags::INDEX_BUFFER_BIT
                    | vk::BufferUsageFlags::VERTEX_BUFFER_BIT,
            }),
        };
        let mem = MemoryPool::new(device, create_info);
        VertexBufferManager {
            mem,
        }
    }

    unsafe fn allocate(&mut self, size: vk::DeviceSize) -> DeviceAlloc {
        // There seems to be no required alignment for vertex/index
        // buffer offsets, so we default to 16.
        let alignment = align(size, 16);
        self.mem.allocate(size, alignment)
    }
}

#[derive(Debug)]
struct Mesh {
    index_count: u32,
    vertex_count: u32,
    triangle_count: u32,
    index: DeviceAlloc,
    pos: DeviceAlloc,
    normal: DeviceAlloc,
}

impl Mesh {
    unsafe fn bind(&self, dt: &vkl::DeviceTable, cmds: vk::CommandBuffer) {
        dt.cmd_bind_index_buffer(
            cmds,                   //commandBuffer
            self.index.buffer(),    //buffer
            self.index.offset(),    //offset
            vk::IndexType::UINT32,  //indexType
        );

        let buffers = [self.pos.buffer(), self.normal.buffer()];
        let offsets = [self.pos.offset(), self.normal.offset()];
        dt.cmd_bind_vertex_buffers(
            cmds,               //commandBuffer
            0,                  //firstBinding
            2,                  //bindingCount
            buffers.as_ptr(),   //pBuffers
            offsets.as_ptr(),   //pOffsets
        );
    }

    unsafe fn bindings_and_attrs() -> (
        &'static [vk::VertexInputBindingDescription],
        &'static [vk::VertexInputAttributeDescription],
    ) {
        let bindings = &[
            vk::VertexInputBindingDescription {
                binding: 0,
                stride: std::mem::size_of::<[f32; 3]>() as _,
                input_rate: vk::VertexInputRate::VERTEX,
            },
            vk::VertexInputBindingDescription {
                binding: 1,
                stride: std::mem::size_of::<[f32; 3]>() as _,
                input_rate: vk::VertexInputRate::VERTEX,
            },
        ];
        let attrs = &[
            vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 0,
            },
            vk::VertexInputAttributeDescription {
                location: 1,
                binding: 1,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 0,
            },
        ];
        (bindings, attrs)
    }
}

unsafe fn unit_cube(buffers: &mut VertexBufferManager) -> Mesh {
    let cube = math::UnitCube;

    let index_buffer = cube.index_buffer();
    let size = std::mem::size_of_val(index_buffer);
    let index = buffers.allocate(size as _);
    (&mut *index.as_slice()).copy_from_slice(index_buffer);


    let pos_buffer = cube.pos_buffer();
    let size = std::mem::size_of_val(pos_buffer);
    let pos = buffers.allocate(size as _);
    (&mut *pos.as_slice()).copy_from_slice(pos_buffer);

    let normal_buffer = cube.normal_buffer();
    let size = std::mem::size_of_val(normal_buffer);
    let normal = buffers.allocate(size as _);
    (&mut *normal.as_slice()).copy_from_slice(normal_buffer);

    Mesh {
        index_count: cube.index_count(),
        vertex_count: cube.vertex_count(),
        triangle_count: cube.triangle_count(),
        index,
        pos,
        normal,
    }
}

// Memory mapped shader storage

#[derive(Debug)]
struct StorageBufferManager {
    mem: MemoryPool,
}

#[derive(Debug)]
struct StorageBlock<T> {
    alloc: DeviceAlloc,
    _p: PhantomData<*mut T>,
}

#[derive(Debug)]
struct StorageVec<T> {
    alloc: DeviceAlloc,
    len: u32,
    _p: PhantomData<*mut [T]>,
}

impl<T> StorageBlock<T> {
    fn new(alloc: DeviceAlloc) -> Self {
        StorageBlock {
            alloc,
            _p: PhantomData,
        }
    }

    #[inline]
    fn data_mut(&mut self) -> &mut T {
        unsafe { &mut *self.alloc.as_block() }
    }

    #[inline]
    fn buffer_info(&self) -> vk::DescriptorBufferInfo {
        self.alloc.buffer_info()
    }
}

impl<T> StorageVec<T> {
    fn new(alloc: DeviceAlloc) -> Self {
        StorageVec {
            alloc,
            len: 0,
            _p: PhantomData,
        }
    }

    #[inline]
    fn len(&self) -> u32 {
        self.len
    }

    #[inline]
    fn capacity(&self) -> u32 {
        (self.alloc.size() / std::mem::size_of::<T>() as vk::DeviceSize) as _
    }

    #[inline]
    fn clear(&mut self) {
        self.len = 0;
    }

    #[inline]
    fn push(&mut self, value: T) {
        unsafe {
            assert_ne!(self.len(), self.capacity());
            let ptr = &mut (*self.alloc.as_slice())[self.len() as usize];
            ptr::write(ptr, value);
            self.len += 1;
        }
    }

    #[allow(dead_code)]
    #[inline]
    fn as_slice(&self) -> &[T] {
        unsafe {
            &(*self.alloc.as_slice())[..self.len() as usize]
        }
    }

    #[allow(dead_code)]
    #[inline]
    fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe {
            &mut (*self.alloc.as_slice())[..self.len() as usize]
        }
    }

    #[inline]
    fn buffer_info(&self) -> vk::DescriptorBufferInfo {
        self.alloc.buffer_info()
    }
}

impl StorageBufferManager {
    unsafe fn new(device: Arc<Device>) -> Self {
        let flags
            = vk::MemoryPropertyFlags::HOST_VISIBLE_BIT
            | vk::MemoryPropertyFlags::HOST_COHERENT_BIT;
        let type_index = find_memory_type(&device, flags).unwrap();
        let create_info = MemoryPoolCreateInfo {
            type_index,
            base_size: 0x10_0000,
            host_mapped: true,
            buffer_map: Some(BufferMapOptions {
                usage: vk::BufferUsageFlags::STORAGE_BUFFER_BIT,
            }),
        };
        let mem = MemoryPool::new(device, create_info);
        StorageBufferManager {
            mem,
        }
    }

    unsafe fn alloc(&mut self, size: u32) -> DeviceAlloc {
        let limits = &self.mem.device().props.limits;
        assert!(size <= limits.max_storage_buffer_range);
        let alignment = limits.min_storage_buffer_offset_alignment;
        self.mem.allocate(size as _, alignment)
    }

    unsafe fn alloc_block<T>(&mut self) -> StorageBlock<T> {
        let alloc = self.alloc(std::mem::size_of::<T>() as _);
        StorageBlock::new(alloc)
    }

    unsafe fn alloc_vec<T>(&mut self, capacity: u32) -> StorageVec<T> {
        let size = capacity * std::mem::size_of::<T>() as u32;
        let alloc = self.alloc(size as _);
        StorageVec::new(alloc)
    }
}

#[derive(Debug)]
#[repr(C)]
struct Instance {
    pos: na::Vector4<f32>,
    fwd: na::Vector4<f32>,
    rgt: na::Vector4<f32>,
    abv: na::Vector4<f32>,
    scale: na::Vector4<f32>,
}

impl From<Transform> for Instance {
    fn from(xform: Transform) -> Self {
        Instance {
            pos: vec4(xform.pos, 1.0),
            rgt: vec4(xform.rot.index((.., 0)).into(), 0.0),
            fwd: vec4(xform.rot.index((.., 1)).into(), 0.0),
            abv: vec4(xform.rot.index((.., 2)).into(), 0.0),
            scale: na::Vector4::from_element(xform.scale),
        }
    }
}

#[derive(Debug)]
#[repr(C)]
struct Camera {
    perspective: na::Matrix4<f32>,
    view: na::Matrix4<f32>,
    pos: na::Vector4<f32>,
    fwd: na::Vector4<f32>,
    dwn: na::Vector4<f32>,
    rgt: na::Vector4<f32>,
}

#[derive(Debug)]
#[repr(C)]
struct SceneGlobals {
    light_dir: na::Vector4<f32>,
    warm_power: na::Vector4<f32>,
    cool_power: na::Vector4<f32>,
    ambient_power: na::Vector4<f32>,
    camera: Camera,
}

#[derive(Debug)]
#[allow(dead_code)]
enum Anim {
    Identity,
    Rotate {
        // axis * angular frequency
        vel: na::Vector3<f32>,
    },
    Pulse {
        range: Range<f32>,
        frequency: f32,
    },
}

impl Anim {
    fn transform_at(&self, time: f32) -> Transform {
        match self {
            Anim::Identity => Transform::identity(),
            Anim::Rotate { vel } => {
                let rot = na::Rotation3::new(time * vel).into_inner();
                Transform {
                    rot,
                    ..Default::default()
                }
            },
            Anim::Pulse { range, frequency } => {
                let weight = 0.5 + 0.5 * f32::cos(frequency * time);
                let scale = weight * range.start + (1.0 - weight) * range.end;
                Transform {
                    scale,
                    ..Default::default()
                }
            },
        }
    }
}

#[derive(Debug)]
struct Object {
    init_xform: Transform,
    anim: Anim,
    mesh: usize,
}

#[derive(Debug)]
struct AppState {
    dt: Arc<vkl::DeviceTable>,
    base: AppBase,
    res: Arc<AppResources>,
    pipelines: GraphicsPipelineManager<PipelineFactory>,
    descriptors: DescriptorAllocator,
    buffers: VertexBufferManager,
    meshes: Vec<Mesh>,
    objects: Vec<Object>,
    storage_mem: StorageBufferManager,
    scene_globals: StorageBlock<SceneGlobals>,
    instances: StorageVec<Instance>,
    desc_pool: DescriptorPool,
    global_set: DescriptorSet,
    cmd_pool: vk::CommandPool,
    cmds: vk::CommandBuffer,
}

impl Drop for AppState {
    fn drop(&mut self) {
        let dt = &*self.dt;
        unsafe {
            dt.device_wait_idle();
            dt.destroy_command_pool(self.cmd_pool, ptr::null());
            dt.destroy_descriptor_pool(self.desc_pool.inner, ptr::null());
        }
    }
}

unsafe fn create_global_set(
    res: &AppResources,
    scene_globals: &StorageBlock<SceneGlobals>,
    instances: &StorageVec<Instance>,
) -> (DescriptorPool, DescriptorSet) {
    let dt = &*res.swapchain.device.table;

    let layout_name = "globals".to_owned();

    let layout = res.set_layouts.get(&layout_name);
    let pool_sizes = layout.counts.pool_sizes(1);
    let flags = Default::default();
    let max_sets = 1;
    let create_info = vk::DescriptorPoolCreateInfo {
        flags,
        max_sets,
        pool_size_count: pool_sizes.len() as _,
        p_pool_sizes: pool_sizes.as_ptr(),
        ..Default::default()
    };
    let mut inner = vk::null();
    dt.create_descriptor_pool(&create_info, ptr::null(), &mut inner)
        .check().unwrap();

    let pool = DescriptorPool {
        layout: layout_name.clone(),
        size: max_sets,
        inner,
    };

    let alloc_info = vk::DescriptorSetAllocateInfo {
        descriptor_pool: pool.inner,
        descriptor_set_count: 1,
        p_set_layouts: &layout.inner,
        ..Default::default()
    };
    let mut inner = vk::null();
    dt.allocate_descriptor_sets(&alloc_info, &mut inner).check().unwrap();

    let set = DescriptorSet {
        layout: layout_name,
        // This field is somewhat nonsensical...
        pool: 0,
        inner,
    };

    let buffer_infos = [
        scene_globals.buffer_info(),
        instances.buffer_info(),
    ];
    let writes = [vk::WriteDescriptorSet {
        dst_set: set.inner,
        descriptor_count: buffer_infos.len() as _,
        descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
        p_buffer_info: buffer_infos.as_ptr(),
        ..Default::default()
    }];
    dt.update_descriptor_sets(
        writes.len() as _,  //descriptorWriteCount
        writes.as_ptr(),    //pDescriptorWrites
        0,                  //descriptorCopyCount
        ptr::null(),        //pDescriptorCopies
    );

    (pool, set)
}

unsafe fn create_objects() -> Vec<Object> {
    vec![{
        let z = na::Vector3::z_axis().into_inner();
        let facing = na::Vector3::new(1.0, 1.0, 1.0);
        let [fwd, abv, rgt] = aiming_basis(facing, z);
        let rot = na::Matrix3::from_columns(&[rgt, fwd, abv]);
        let pos = na::Vector3::new(4.0, -1.0, -1.0);
        let scale = 0.75;
        let init_xform = Transform { pos, rot, scale };

        let axis = na::Vector3::new(-1.0, 1.0, -1.0).normalize();
        let freq = 0.25;
        let vel = 2.0 * PI * freq * axis;
        let anim = Anim::Rotate { vel };

        Object {
            init_xform,
            anim,
            mesh: 0,
        }
    }]
}

unsafe fn init_state(res: Arc<AppResources>) -> AppState {
    let gfx_queue = Arc::clone(&res.queues[0][0]);
    let device = Arc::clone(&gfx_queue.device);
    let dt = Arc::clone(&device.table);

    let base = AppBase::new(Arc::clone(&res));

    let factory = PipelineFactory::new(Arc::clone(&res));
    let pipelines = GraphicsPipelineManager::new(Arc::clone(&device), factory);

    let create_info = vk::CommandPoolCreateInfo {
        flags: vk::CommandPoolCreateFlags::TRANSIENT_BIT
            | vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER_BIT,
        queue_family_index: gfx_queue.family.index,
        ..Default::default()
    };
    let mut cmd_pool = vk::null();
    dt.create_command_pool(&create_info, ptr::null(), &mut cmd_pool);

    let mut cmds = vk::null();
    let alloc_info = vk::CommandBufferAllocateInfo {
        command_pool: cmd_pool,
        command_buffer_count: 1,
        ..Default::default()
    };
    dt.allocate_command_buffers(&alloc_info, &mut cmds);

    let descriptors = DescriptorAllocator::new(
        Arc::clone(&device),
        Arc::clone(&res.set_layouts),
    );

    let mut buffers = VertexBufferManager::new(Arc::clone(&device));
    let meshes = vec![unit_cube(&mut buffers)];

    let objects = create_objects();

    let mut storage_mem = StorageBufferManager::new(Arc::clone(&device));

    let scene_globals = storage_mem.alloc_block();
    let instances = storage_mem.alloc_vec(256);

    let (desc_pool, global_set) =
        create_global_set(&res, &scene_globals, &instances);

    AppState {
        dt,
        base,
        res,
        pipelines,
        descriptors,
        buffers,
        meshes,
        objects,
        storage_mem,
        scene_globals,
        instances,
        desc_pool,
        global_set,
        cmd_pool,
        cmds,
    }
}

impl AppState {
    unsafe fn update_globals(&mut self) {
        // Camera
        let viewport = self.res.swapchain.viewport();
        let aspect = viewport.width / viewport.height;
        let fovy = 90.0f32.to_radians();
        let tan_fovy2 = (0.5 * fovy).tan();
        let perspective = PerspectiveTransform {
            z_near: 10e-3,
            z_far: 10e3,
            aspect,
            tan_fovy2,
            min_depth: viewport.min_depth,
            max_depth: viewport.max_depth,
        }.to_matrix();

        let dir = na::Vector3::x_axis().into_inner();
        let up = na::Vector3::z_axis().into_inner();
        let [fwd, abv, rgt] = aiming_basis(dir, up);
        let dwn = -abv;
        let pos = na::zero();
        let rot = na::Matrix3::from_columns(&[rgt, dwn, fwd]);
        let view = Transform {
            rot,
            pos,
            scale: 1.0,
        }.inverse().to_matrix();

        let camera = Camera {
            perspective,
            view,
            pos: vec4(pos, 1.0),
            fwd: vec4(fwd, 0.0),
            dwn: vec4(dwn, 0.0),
            rgt: vec4(rgt, 0.0),
        };

        // Lighting
        let light_dir = na::Vector3::new(1.0, -1.0, -1.0).normalize();
        let warm_power = na::Vector4::new(0.9, 0.5, 0.1, 0.0);
        let cool_power = na::Vector4::new(0.5, 0.1, 0.9, 0.0);
        let ambient_power = na::Vector4::new(0.05, 0.05, 0.1, 0.0);

        let globals = SceneGlobals {
            light_dir: vec4(light_dir, 0.0),
            warm_power,
            cool_power,
            ambient_power,
            camera,
        };
        *self.scene_globals.data_mut() = globals;
    }

    unsafe fn update_objects(&mut self) {
        let time = (self.base.frame_start - self.base.start).as_secs_f32();

        self.instances.clear();
        for object in self.objects.iter() {
            let xform = object.init_xform * object.anim.transform_at(time);
            self.instances.push(xform.into());
        }
    }

    unsafe fn record_cmds(&mut self) {
        let dt = &*self.dt;

        let cmds = self.cmds;
        let begin_info = vk::CommandBufferBeginInfo {
            flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT_BIT,
            ..Default::default()
        };
        dt.begin_command_buffer(cmds, &begin_info);

        let render_pass = self.res.render_passes.get("forward").inner;
        let framebuffer = self.base.cur_framebuffer();
        let render_area = self.res.framebuffers.rect();
        let clear_values = [vk::ClearValue {
            color: vk::ClearColorValue { float_32: [0.0, 0.0, 0.0, 1.0] },
        }];
        let begin_info = vk::RenderPassBeginInfo {
            render_pass,
            framebuffer,
            render_area,
            clear_value_count: clear_values.len() as _,
            p_clear_values: clear_values.as_ptr(),
            ..Default::default()
        };
        let contents = vk::SubpassContents::INLINE;
        dt.cmd_begin_render_pass(cmds, &begin_info, contents);

        let pipeline = self.pipelines.get(&()).inner;
        dt.cmd_bind_pipeline(cmds, vk::PipelineBindPoint::GRAPHICS, pipeline);

        let layout = self.res.pipe_layouts.get("globals").inner;
        let sets = [self.global_set.inner];
        dt.cmd_bind_descriptor_sets(
            cmds,                               //commandBuffer
            vk::PipelineBindPoint::GRAPHICS,    //pipelineBindPoint
            layout,                             //layout
            0,                                  //firstSet
            sets.len() as _,                    //descriptorSetCount
            sets.as_ptr(),                      //pDescriptorSets
            0,                                  //dynamicOffsetCount
            ptr::null(),                        //pDynamicOffsets
        );

        let mesh = &self.meshes[0];
        mesh.bind(dt, cmds);
        dt.cmd_draw_indexed(
            cmds,
            mesh.index_count,
            self.instances.len(),
            0,
            0,
            0,
        );

        dt.cmd_end_render_pass(cmds);

        dt.end_command_buffer(cmds);
    }

    unsafe fn submit_cmds(&mut self) {
        let cmd_bufs = [self.cmds];
        let wait_sems = [self.base.acquire_sem];
        let wait_masks = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT_BIT];
        let sig_sems = [self.base.render_sem];
        let submit_infos = [vk::SubmitInfo {
            command_buffer_count: cmd_bufs.len() as _,
            p_command_buffers: cmd_bufs.as_ptr(),
            wait_semaphore_count: wait_sems.len() as _,
            p_wait_semaphores: wait_sems.as_ptr(),
            p_wait_dst_stage_mask: wait_masks.as_ptr(),
            signal_semaphore_count: sig_sems.len() as _,
            p_signal_semaphores: sig_sems.as_ptr(),
            ..Default::default()
        }];
        self.base.gfx_queue.submit(&submit_infos[..], self.base.render_fence);
    }
}

unsafe fn render_main(ev_proxy: window::EventLoopProxy) {
    let (swapchain, queues) = init_video(&ev_proxy, "scene demo");
    let res = Arc::new(init_resources(swapchain, queues));
    let mut state = init_state(res);

    while !state.res.window.should_close() {
        state.base.acquire_next_image();
        state.base.wait_for_render();
        state.update_globals();
        state.update_objects();
        state.record_cmds();
        state.submit_cmds();
        state.base.present();
    }
}

fn main() {
    unsafe { with_event_loop(|proxy| render_main(proxy)); }
}
