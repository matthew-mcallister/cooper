use std::sync::Arc;

use base::ByPtr;
use derive_more::Display;
use device::{
    DescriptorSet, DescriptorType, ImageView, Lifetime, SetLayoutBinding,
    SetLayoutDesc,
};
use enum_map::EnumMap;
use fnv::FnvHashMap as HashMap;
use log::trace;
use smallvec::smallvec;

use crate::Globals;
use crate::resource::{ResourceState, ResourceSystem};
use crate::util::SmallVec;
use super::*;

#[derive(Clone, Copy, Debug, Default, Display, Eq, PartialEq)]
#[display(fmt = "resource not available on device")]
crate struct ResourceUnavailable;
impl std::error::Error for ResourceUnavailable {}
impl_from_via_default!(ResourceUnavailable, std::option::NoneError);

#[derive(Clone, Debug)]
struct ImageBindingState {
    crate view: Arc<ImageView>,
    crate sampler: Arc<Sampler>,
}

type MaterialImageState = EnumMap<MaterialImage, ImageBindingState>;

/// A loaded material with all shader bindings available.
#[derive(Debug)]
crate struct Material {
    def: Arc<MaterialDef>,
    images: MaterialImageState,
    desc: Option<DescriptorSet>,
}

#[derive(Debug)]
pub(super) struct MaterialStateTable {
    globals: Arc<Globals>,
    materials: HashMap<ByPtr<Arc<MaterialDef>>, Arc<Material>>,
}

#[allow(dead_code)]
impl Material {
    crate fn def(&self) -> &Arc<MaterialDef> {
        &self.def
    }

    crate fn desc(&self) -> Option<&DescriptorSet> {
        self.desc.as_ref()
    }

    crate fn select_shaders(&self) -> ShaderStageMap {
        self.def.shader_stages().clone()
    }
}

impl MaterialStateTable {
    pub(super) fn new(globals: &Arc<Globals>) -> Self {
        Self {
            globals: Arc::clone(globals),
            materials: Default::default(),
        }
    }

    #[allow(dead_code)]
    pub(super) fn get_state(&self, def: &Arc<MaterialDef>) -> ResourceState {
        tryopt!(resource_state(self.materials.get(ByPtr::by_ptr(def))))
            .unwrap_or(ResourceState::Unavailable)
    }

    pub(super) fn get_or_create(
        &mut self,
        state: &SystemState,
        resources: &ResourceSystem,
        def: &Arc<MaterialDef>,
    ) -> Result<&Arc<Material>, ResourceUnavailable> {
        // borrowck is braindead
        let materials = unsafe { &*(&self.materials as *const HashMap<_, _>) };
        tryopt! { return Ok(materials.get(ByPtr::by_ptr(def))?); };

        let material = create_material(state, &self.globals, resources, def)?;
        let material = self.materials.entry(Arc::clone(def).into())
            .insert(Arc::new(material)).into_mut();
        Ok(material)
    }
}

#[allow(dead_code)]
fn resource_state(state: Option<&Arc<Material>>) -> ResourceState {
    if state.is_some() {
        ResourceState::Available
    } else {
        ResourceState::Unavailable
    }
}

fn create_material(
    state: &SystemState,
    globals: &Globals,
    resources: &ResourceSystem,
    def: &Arc<MaterialDef>,
) -> Option<Material> {
    let images = def.image_bindings.iter()
        .map(|(k, binding)| {
            let image = resources.get_image(&binding.image)?;
            Some((k, (binding, image)))
        }: Option<_>)
        .collect::<Option<_>>()?;
    let images = create_image_states(state, globals, images);
    let desc = create_descriptor_set(state, &images);
    Some(Material {
        def: Arc::clone(def),
        images,
        desc,
    })
}

fn create_image_states(
    state: &SystemState,
    globals: &Globals,
    bindings: PartialEnumMap<MaterialImage, (&ImageBindingDesc, &Arc<Image>)>,
) -> MaterialImageState {
    trace!("create_image_states(bindings: {:?})", bindings);
    let default_image_state = || ImageBindingState {
        view: Arc::clone(&globals.immediate_image_2d),
        sampler: Arc::clone(&globals.empty_sampler),
    };
    EnumMap::from(|k| {
        let state = if let Some((binding, image)) = bindings.get(k) {
            create_image_state(state, binding, Arc::clone(image))
        } else { default_image_state() };
        state
    })
}

fn create_image_state(
    state: &SystemState,
    binding: &ImageBindingDesc,
    image: Arc<Image>,
) -> ImageBindingState {
    // TODO: We currently create a new ImageView for every image,
    // but it may be worthwhile to cache and share views.
    let view = unsafe { Arc::new(ImageView::new(
        image,
        binding.flags,
        binding.image.format(),
        Default::default(),
        binding.subresources,
    )) };
    let sampler = state.samplers.get_or_create(&binding.sampler_state)
        .into_owned();
    ImageBindingState { view, sampler }
}

fn create_descriptor_set(
    state: &SystemState,
    images: &MaterialImageState,
) -> Option<DescriptorSet> {
    let samplers =
        images.values().map(|img| Arc::clone(&img.sampler)).collect();
    let layout = state.set_layouts.get_or_create(&SetLayoutDesc {
        bindings: smallvec![SetLayoutBinding {
            binding: 0,
            ty: DescriptorType::CombinedImageSampler,
            count: images.len() as u32,
            stage_flags: vk::ShaderStageFlags::FRAGMENT_BIT,
            samplers: Some(samplers),
        }],
        ..Default::default()
    }).into_owned();

    let mut set =
        state.descriptors.alloc(Lifetime::Static, &layout);
    let views: SmallVec<_, {MaterialImage::SIZE}> =
        images.values().map(|img| &img.view).collect();
    unsafe {
        set.write_images(
            0, 0,
            &views,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            None,
        );
    }
    Some(set)
}
