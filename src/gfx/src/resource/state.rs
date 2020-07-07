use std::sync::Arc;

use base::ByPtr;
use fnv::FnvHashMap as HashMap;

use crate::{DeviceAlloc, ImageHeap, Image};

#[derive(Debug)]
crate struct ResourceStateTable {
    images: HashMap<ByPtr<Arc<Image>>, ResourceState>,
}

#[derive(Debug)]
crate struct ResourceState {
    crate alloc: Option<DeviceAlloc>,
    crate batch: u64,
}

impl ResourceState {
    crate fn available(&self, avail_batch: u64) -> bool {
        self.alloc.is_some() & (self.batch <= avail_batch)
    }
}

impl ResourceStateTable {
    crate fn new() -> Self {
        Self { images: Default::default() }
    }

    crate fn register(&mut self, image: Arc<Image>) {
        assert!(image.alloc().is_none());
        self.images.insert(image.into(), ResourceState {
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
