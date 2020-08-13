use std::sync::Arc;

use base::ByPtr;
use derive_more::Display;
use device::{
    DescriptorSet, DescriptorType, GraphicsPipeline, GraphicsPipelineDesc,
    ImageView, Lifetime, SetLayoutBinding, SetLayoutDesc,
};
use enum_map::EnumMap;
use fnv::FnvHashMap as HashMap;
use smallvec::smallvec;
use prelude::tryopt;

use crate::Globals;
use crate::resource::ResourceSystem;
use crate::util::SmallVec;
use super::*;

#[derive(Clone, Copy, Debug, Default, Display, Eq, PartialEq)]
#[display(fmt = "resource not available on device")]
crate struct ResourceUnavailable;
impl std::error::Error for ResourceUnavailable {}
impl_from_via_default!(ResourceUnavailable, std::option::NoneError);

type MaterialImageState = EnumMap<MaterialImage, Arc<ImageView>>;

#[derive(Debug)]
struct MaterialResources {
    images: MaterialImageState,
    desc: Arc<DescriptorSet>,
}

// Idea: each subpass has its own pipeline cache. When a material
// is created, any subpass in which it will be used has to create a
// pipeline, e.g. depth pass, deferred pass, translucent/forward pass.
#[derive(Debug, Default)]
crate struct MaterialState {
    pipeline: Option<Arc<GraphicsPipeline>>,
    resources: Option<MaterialResources>,
}

#[derive(Debug)]
crate struct MaterialStateTable {
    globals: Arc<Globals>,
    defs: HashMap<Arc<MaterialDesc>, Arc<MaterialDef>>,
    materials: HashMap<ByPtr<Arc<MaterialDef>>, MaterialState>,
}

impl MaterialState {
    crate fn pipeline(&self) -> Option<&Arc<GraphicsPipeline>> {
        self.pipeline.as_ref()
    }

    crate fn desc(&self) -> Option<&Arc<DescriptorSet>> {
        Some(&self.resources.as_ref()?.desc)
    }
}

impl MaterialStateTable {
    crate fn new(_state: &SystemState, globals: &Arc<Globals>) -> Self {
        Self {
            globals: Arc::clone(globals),
            defs: Default::default(),
            materials: Default::default(),
        }
    }

    crate fn define(
        &mut self,
        state: &mut SystemState,
        desc: &MaterialDesc,
    ) -> Arc<MaterialDef> {
        tryopt! { return Arc::clone(self.defs.get(desc)?) };
        let desc = Arc::new(desc.clone());
        let set_layout = create_set_layout(
            state, &self.globals, &desc.image_bindings);
        let def = Arc::new(MaterialDef { desc, set_layout });
        self.materials.insert(Arc::clone(&def).into(), Default::default());
        def
    }

    crate fn update_resolved_resources(
        &mut self,
        state: &mut SystemState,
        resources: &ResourceSystem,
    ) {
        for (def, mat) in self.materials.iter_mut()
            .filter(|(_, mat)| mat.resources.is_none())
        {
            mat.resources = try_resolve_material_resources(
                state, &self.globals, resources, ByPtr::by_value(def));
        }
    }

    // TODO: Multiple pipelines per material keyed by the base pipeline
    // descriptor.
    crate unsafe fn create_pipelines(
        &mut self,
        state: &mut SystemState,
        base_desc: &mut GraphicsPipelineDesc,
    ) {
        for (def, mat) in self.materials.iter_mut()
            .filter(|(_, mat)| mat.pipeline.is_none())
        {
            mat.pipeline = Some(create_pipeline(state, base_desc, def));
        }
    }

    crate fn get(&self, def: &Arc<MaterialDef>) -> &MaterialState {
        &self.materials[ByPtr::by_ptr(def)]
    }
}

unsafe fn create_pipeline(
    state: &mut SystemState,
    desc: &mut GraphicsPipelineDesc,
    def: &MaterialDef,
) -> Arc<GraphicsPipeline> {
    desc.stages = def.stages().clone();
    desc.vertex_layout = def.vertex_layout().clone();
    // A little clunky, but should be flexible enough
    desc.layout.set_layouts[1] = Arc::clone(def.set_layout());
    Arc::clone(state.pipelines.get_or_create_committed_gfx(&desc))
}

// TODO: Resolve resources as they become available rather than
// checking all resources every time.
fn try_resolve_material_resources(
    state: &mut SystemState,
    globals: &Globals,
    resources: &ResourceSystem,
    def: &Arc<MaterialDef>,
) -> Option<MaterialResources> {
    let images: PartialEnumMap<_, _> = def.image_bindings().iter()
        .map(|(k, binding)| {
            let image = resources.get_image(&binding.image)?;
            Some((k, (binding, image)))
        }: Option<_>)
        .collect::<Option<_>>()?;
    let default_view = &globals.immediate_image_2d;
    let images = EnumMap::from(|k| {
        let state = if let Some((binding, image)) = images.get(k) {
            create_image_view(binding, Arc::clone(image))
        } else { Arc::clone(default_view) };
        state
    });
    let desc = Arc::new(create_descriptor_set(
        state, def.set_layout(), &images));
    Some(MaterialResources { images, desc })
}

fn create_image_view(
    binding: &ImageBindingDesc,
    image: Arc<Image>,
) -> Arc<ImageView> {
    // TODO: We currently create a new ImageView for every image,
    // but it may be worthwhile to cache and share views.
    unsafe { Arc::new(ImageView::new(
        image,
        Default::default(),
        binding.image.format(),
        Default::default(),
        binding.subresources,
    )) }
}

pub(super) fn create_set_layout(
    state: &mut SystemState,
    globals: &Globals,
    bindings: &MaterialImageBindings,
) -> Arc<DescriptorSetLayout> {
    let default_sampler = &globals.empty_sampler;
    let samplers = MaterialImage::values().map(|k| {
        if let Some(binding) = bindings.get(k) {
            let desc = &binding.sampler_state;
            Arc::clone(state.samplers.get_or_create_committed(desc))
        } else {
            Arc::clone(default_sampler)
        }
    }).collect();
    state.set_layouts.get_or_create(&SetLayoutDesc {
        bindings: smallvec![SetLayoutBinding {
            binding: 0,
            ty: DescriptorType::CombinedImageSampler,
            count: bindings.capacity() as u32,
            stage_flags: vk::ShaderStageFlags::FRAGMENT_BIT,
            samplers: Some(samplers),
        }],
    }).into_owned()
}

fn create_descriptor_set(
    state: &SystemState,
    layout: &Arc<DescriptorSetLayout>,
    images: &MaterialImageState,
) -> DescriptorSet {
    let mut set = state.descriptors.alloc(Lifetime::Static, &layout);
    let views: SmallVec<_, {MaterialImage::SIZE}> = images.values().collect();
    unsafe {
        set.write_images(
            0, 0,
            &views,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            None,
        );
    }
    set
}
