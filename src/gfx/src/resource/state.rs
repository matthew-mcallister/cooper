use std::sync::Arc;

use base::ByPtr;
use fnv::FnvHashMap as HashMap;
use log::trace;
use prelude::*;

use crate::{Image, ImageDef, ImageHeap, ResourceState};

#[derive(Debug)]
crate struct ResourceStateTable {
    images: HashMap<ByPtr<Arc<ImageDef>>, ResourceInfo>,
}

// TODO: Is it possible to store this on the ImageDef itself while still
// obtaining exclusive access to it?
#[derive(Debug, Default)]
struct ResourceInfo {
    resource: Option<Arc<Image>>,
    batch: u64,
}

impl ResourceInfo {
    fn state(&self, avail_batch: u64) -> ResourceState {
        match (self.resource.is_some(), self.batch <= avail_batch) {
            (true, true) => ResourceState::Available,
            (true, false) => ResourceState::Pending,
            (false, _) => ResourceState::Unavailable,
        }
    }
}

impl ResourceStateTable {
    crate fn new() -> Self {
        Self { images: Default::default() }
    }

    fn get_or_init(&mut self, image: &Arc<ImageDef>) -> &mut ResourceInfo {
        let image = ByPtr::by_ptr(image);
        self.images.raw_entry_mut().from_key(&image)
            .or_insert_with(|| (image.clone(), Default::default())).1
    }

    crate fn get_state(&self, image: &Arc<ImageDef>, avail_batch: u64) ->
        ResourceState
    {
        tryopt!(self.images.get(ByPtr::by_ptr(image))?.state(avail_batch))
            .unwrap_or(ResourceState::Unavailable)
    }

    crate fn prepare_for_upload(
        &mut self,
        image: &Arc<ImageDef>,
        batch: u64,
        heap: &ImageHeap,
    ) -> &Arc<Image> {
        trace!(
            "ResourceStateTable::prepare_for_upload(image: {:?}, batch: {:?})",
            image, batch,
        );

        let info = self.get_or_init(image);
        info.batch = batch;
        info.resource.get_or_insert_with(|| {
            Arc::new(Image::new(heap, Arc::clone(image)))
        })
    }

    crate fn get_image(&self, image: &Arc<ImageDef>, avail_batch: u64) ->
        Option<&Arc<Image>>
    {
        trace!(
            "ResourceStateTable::get_image(image: {:?}, avail_batch: {:?})",
            image, avail_batch,
        );
        let info = self.images.get(ByPtr::by_ptr(image))?;
        guard(info.state(avail_batch) == ResourceState::Available)?;
        info.resource.as_ref()
    }
}
