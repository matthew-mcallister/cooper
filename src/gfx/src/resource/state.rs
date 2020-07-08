use std::sync::Arc;

use base::ByPtr;
use fnv::FnvHashMap as HashMap;

use crate::{DeviceAlloc, ImageHeap, Image};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
crate enum ResourceState {
    Available,
    Pending,
    Unavailable,
}

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

    crate fn register(&mut self, image: Arc<Image>) {
        assert!(image.alloc().is_none());
        self.images.insert(image.into(), ResourceInfo {
            alloc: None,
            batch: 0,
        });
    }

    crate unsafe fn alloc(
        &mut self,
        image: &Arc<Image>,
        batch: u64,
        heap: &ImageHeap,
    ) {
        let state = self.images.get_mut(ByPtr::by_ptr(image)).unwrap();

        state.batch = batch;

        let alloc = heap.bind(image.inner());
        state.alloc = Some(alloc);
    }

    crate fn touch(&mut self, image: &Arc<Image>) {
        let image = &self.images[ByPtr::by_ptr(image)];
        assert!(image.alloc.is_some());
    }
}
