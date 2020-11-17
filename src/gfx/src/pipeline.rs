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

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct PatchDef {
    pub mesh: RenderMesh,
    pub stages: ShaderStageMap,
    /// Binds image handles to material image slots. Slots without
    /// explicit bindings will have a default image/sampler pair bound
    /// to them.
    pub image_bindings: MaterialImageBindings,
    pub cull_mode: CullMode,
}

#[derive(Debug)]
crate struct Patch {
    crate index: Option<IndexBuffer<BufferAlloc>>,
    crate attrs: SmallVec<AttrBuffer<BufferAlloc>, 6>,
    crate pipeline: Arc<GraphicsPipeline>,
    crate resources: MaterialResources,
}

#[derive(Debug)]
crate struct PatchTable {
    patches: HashMap<ByPtr<Arc<PatchDef>>, Patch>,
}

impl PatchTable {
    crate fn new() -> Self {
        Self {
            patches: Default::default(),
        }
    }

    fn get(&self, def: &Arc<PatchDef>) -> Option<&MaterialState> {
        self.materials.get(ByPtr::by_ptr(def))
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
    desc.cull_mode = def.cull_mode();
    Arc::clone(state.pipelines.get_or_create_committed_gfx(&desc))
}

// If all images used in a material are available to shaders, prepares
// a descriptor set that can be passed directly to shaders for
// rendering. Fails if any image is unavailable.
fn try_resolve_material_resources(
    state: &mut SystemState,
    globals: &Globals,
    resources: &ResourceSystem,
    def: &Arc<MaterialDef>,
) -> Option<MaterialResources> {
    // TODO: EnumMap doesn't allow fallible creation...
    let mut images = PartialEnumMap::new();
    for k in MaterialImage::values() {
        let def = if let Some(binding) = def.image_bindings().get(k) {
            &binding.image
        } else {
            &globals.empty_image_2d
        };
        images.insert(k, resources.get_image(def)?);
    }
    // TODO: This creates many redundant image views
    let images = EnumMap::from(|k| images[k].create_full_view());
    let desc = Arc::new(create_descriptor_set(
        state, def.set_layout(), &images));
    Some(MaterialResources { images, desc })
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
