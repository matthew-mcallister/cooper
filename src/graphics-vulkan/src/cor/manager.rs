use std::ptr;
use std::sync::Arc;

use ccore::name::*;
use fnv::FnvHashMap;
use owning_ref::OwningRef;
use parking_lot::RwLock;
use prelude::*;

use crate::*;
use super::objects::*;
use super::pipeline::*;

// TODO: Iffy on this type. Maybe there should be a single global
// "device manager" type that is nigh-oblivious to synchronization
// (similar to VkDevice) and proxy types should handle proper ownership
// and synchronization but not storage or driver logic.
#[derive(Debug)]
crate struct CoreData {
    pub(super) config: Config,
    pub(super) device: Arc<Device>,
    pub(super) gfx_queue: Arc<Queue>,
    pub(super) set_layouts: FnvHashMap<Name, DescriptorSetLayout>,
    pub(super) descriptors: DescriptorAllocator,
    pub(super) pipe_layouts: FnvHashMap<Name, PipelineLayout>,
    pub(super) shaders: FnvHashMap<Name, Shader>,
    pub(super) passes: FnvHashMap<Name, RenderPass>,
    pub(super) pipelines: RwLock<FnvHashMap<PipelineDesc, GraphicsPipeline>>,
}

impl Drop for CoreData {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            for layout in self.set_layouts.values() {
                dt.destroy_descriptor_set_layout(layout.inner(), ptr::null());
            }
            for layout in self.pipe_layouts.values() {
                dt.destroy_pipeline_layout(layout.inner(), ptr::null());
            }
            for shader in self.shaders.values() {
                dt.destroy_shader_module(shader.inner(), ptr::null());
            }
            for pass in self.passes.values() {
                dt.destroy_render_pass(pass.inner(), ptr::null());
            }
            for pipeline in self.pipelines.get_mut().values() {
                dt.destroy_pipeline(pipeline.inner(), ptr::null());
            }
        }
    }
}

impl CoreData {
    crate fn new(
        device: Arc<Device>,
        queues: &Vec<Vec<Arc<Queue>>>,
        config: Config,
    ) -> Self {
        let pipelines = FnvHashMap::<PipelineDesc, _>::default();
        CoreData {
            config,
            gfx_queue: Arc::clone(&queues[0][0]),
            descriptors: DescriptorAllocator::new(Arc::clone(&device)),
            set_layouts: Default::default(),
            pipe_layouts: Default::default(),
            shaders: Default::default(),
            passes: Default::default(),
            pipelines: RwLock::new(pipelines),
            device,
        }
    }

    crate fn init(&mut self) {
        unsafe {
            create_set_layouts(self);
            create_pipe_layouts(self);
            create_render_passes(self);
            create_shaders(self);
        }
    }

    crate unsafe fn insert_set_layout(
        &mut self,
        name: Name,
        layout: DescriptorSetLayout,
    ) {
        self.device.set_debug_name(layout.inner(), name.as_str());
        insert_unique!(&mut self.set_layouts, name, layout);
    }

    crate fn config(&self) -> &Config {
        &self.config
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    crate fn gfx_queue(&self) -> &Arc<Queue> {
        &self.gfx_queue
    }

    crate fn get_set_layout(&self, name: Name) -> &DescriptorSetLayout {
        &self.set_layouts[&name]
    }

    crate fn alloc_desc_set(&self, set_layout: Name) -> DescriptorSet {
        unsafe {
            if let Some(set) = self.descriptors.allocate(set_layout) {
                return set;
            }

            let layout = self.get_set_layout(set_layout);
            self.descriptors.insert_alloc(set_layout.to_owned(), layout)
        }
    }

    crate fn free_desc_set(&self, set: DescriptorSet) {
        self.descriptors.free(set)
    }

    crate fn get_pipe_layout(&self, name: Name) -> &PipelineLayout {
        &self.pipe_layouts[&name]
    }

    crate fn get_shader(&self, name: Name) -> &Shader {
        &self.shaders[&name]
    }

    crate fn get_pass(&self, name: Name) -> &RenderPass {
        &self.passes[&name]
    }

    crate fn get_pipeline(&self, desc: &PipelineDesc) ->
        impl std::ops::Deref<Target = GraphicsPipeline> + '_
    {
        // TODO: Could parallelize pipeline creation, probably using
        // monitor pattern.
        let pipelines = &self.pipelines;

        // Try to fetch an existing pipeline
        let res = OwningRef::new(pipelines.read())
            .try_map(|pipes| pipes.get(&desc).ok_or(()));
        if let Ok(res) = res { return res; }

        // Not found; create the missing pipeline
        {
            let mut pipelines = pipelines.write();
            let pipe = unsafe { create_graphics_pipeline(self, desc) };
            pipelines.insert(desc.clone(), pipe);
        }

        OwningRef::new(pipelines.read())
            .map(|pipes| &pipes[desc])
    }
}
