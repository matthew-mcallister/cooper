use std::sync::Arc;

use base::ByPtr;
use fnv::FnvHashMap as HashMap;
use log::trace;

use crate::{DeviceAlloc, ImageHeap, Image, ResourceState};

#[derive(Debug)]
crate struct ResourceStateTable {
    images: HashMap<ByPtr<Arc<Image>>, ResourceInfo>,
}

#[derive(Debug)]
struct ResourceInfo {
    alloc: Option<DeviceAlloc>,
    batch: u64,
}

impl ResourceInfo {
    fn state(&self, avail_batch: u64) -> ResourceState {
        match (self.alloc.is_some(), self.batch <= avail_batch) {
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

    crate fn register(&mut self, image: &Arc<Image>) {
        assert!(image.alloc().is_none());
        self.images.entry(Arc::clone(image).into()).or_insert(ResourceInfo {
            alloc: None,
            batch: 0,
        });
    }

    crate fn get_state(&self, image: &Arc<Image>, avail_batch: u64) ->
        ResourceState
    {
        try_opt!(self.images.get(ByPtr::by_ptr(image))?.state(avail_batch))
            .unwrap_or(ResourceState::Unavailable)
    }

    crate fn prepare_for_upload(
        &mut self,
        image: &Arc<Image>,
        batch: u64,
        heap: &ImageHeap,
    ) {
        trace!(
            "ResourceStateTable::prepare_for_upload(image: {:?}, batch: {:?})",
            image, batch,
        );

        let info = self.images.get_mut(ByPtr::by_ptr(image)).unwrap();

        info.batch = batch;

        if info.alloc.is_none() {
            info.alloc = unsafe { Some(heap.bind(image.inner())) };
        }
    }
}
