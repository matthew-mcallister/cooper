use std::sync::Arc;

use base::ByPtr;
use derive_more::Display;
use fnv::FnvHashMap as HashMap;

use crate::impl_from_via_default;
use crate::device::ImageView;
use crate::resource::{ResourceState, ResourceSystem};
use super::*;

#[derive(Clone, Copy, Debug, Default, Display, Eq, PartialEq)]
#[display(fmt = "resource not available on device")]
crate struct ResourceUnavailable;
impl std::error::Error for ResourceUnavailable {}
impl_from_via_default!(ResourceUnavailable, std::option::NoneError);

#[derive(Debug)]
pub(super) struct MaterialStateTable {
    materials: HashMap<ByPtr<Arc<MaterialDef>>, Arc<Material>>,
}

impl MaterialStateTable {
    pub(super) fn new() -> Self {
        Self {
            materials: Default::default(),
        }
    }

    pub(super) fn get_state(&self, def: &Arc<MaterialDef>) -> ResourceState {
        try_opt!(resource_state(self.materials.get(ByPtr::by_ptr(def))))
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
        try_opt! { return Ok(materials.get(ByPtr::by_ptr(def))?); };

        let material = create_material(state, resources, Arc::clone(&def))?;
        let material = self.materials.entry(Arc::clone(def).into())
            .insert(Arc::new(material)).into_mut();
        Ok(material)
    }
}

fn resource_state(state: Option<&Arc<Material>>) -> ResourceState {
    if state.is_some() {
        ResourceState::Available
    } else {
        ResourceState::Unavailable
    }
}

fn create_material(
    state: &SystemState,
    resources: &ResourceSystem,
    def: Arc<MaterialDef>,
) -> Option<Material> {
    let images: PartialEnumMap<_, &Arc<Image>> = def.image_bindings.iter()
        .filter_map(|(k, binding)| {
            Some((k, resources.get_image(&binding.image)?))
        })
        .collect();
    let images = bind_images(images, &def.image_bindings);
    let desc = def.factory.create_descriptor_set(state, &images);
    Some(Material {
        def,
        images,
        desc,
    })
}

fn bind_images(
    images: PartialEnumMap<MaterialImage, &Arc<Image>>,
    bindings: &MaterialImageBindings,
) -> MaterialImageState {
    bindings.iter().map(|(name, binding)| {
        // TODO: We currently create a new ImageView for every image,
        // but they could conceivably be cached and shared.
        let view = unsafe { Arc::new(ImageView::new(
            Arc::clone(images[name]),
            binding.flags,
            binding.image.format(),
            Default::default(),
            binding.subresources,
        )) };
        (name, ImageBindingState {
            view,
            sampler: Arc::clone(&binding.sampler),
        })
    }).collect()
}
