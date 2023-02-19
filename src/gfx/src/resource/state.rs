use std::sync::Arc;

use base::ByPtr;
use derivative::Derivative;
use device::{fmt_named, BufferAlloc, BufferHeap, Image, ImageDef, ImageHeap};
use fnv::FnvHashMap as HashMap;
use log::trace;
use prelude::*;

use super::*;

// TODO: Maybe allow purging buffers/images with no external references.
#[derive(Debug)]
pub(crate) struct ResourceStateTable {
    avail_batch: u64,
    buffers: HashMap<ByPtr<Arc<BufferDef>>, ResourceInfo<BufferAlloc>>,
    images: HashMap<ByPtr<Arc<ImageDef>>, ResourceInfo<Image>>,
}

#[derive(Debug, Derivative)]
#[derivative(Default(bound = ""))]
struct ResourceInfo<R> {
    resource: Option<Arc<R>>,
    batch: u64,
}

impl<R> ResourceInfo<R> {
    fn state(&self, avail_batch: u64) -> ResourceState {
        match (self.resource.is_some(), self.batch <= avail_batch) {
            (true, true) => ResourceState::Available,
            (true, false) => ResourceState::Pending,
            (false, _) => ResourceState::Unavailable,
        }
    }
}

impl ResourceStateTable {
    pub(crate) fn new() -> Self {
        Self {
            avail_batch: 0,
            buffers: Default::default(),
            images: Default::default(),
        }
    }

    fn get_or_init_image(&mut self, image: &Arc<ImageDef>) -> &mut ResourceInfo<Image> {
        let image = ByPtr::by_ptr(image);
        self.images
            .raw_entry_mut()
            .from_key(&image)
            .or_insert_with(|| (ByPtr::clone(&image), Default::default()))
            .1
    }

    fn get_or_init_buffer(&mut self, buffer: &Arc<BufferDef>) -> &mut ResourceInfo<BufferAlloc> {
        let buffer = ByPtr::by_ptr(buffer);
        self.buffers
            .raw_entry_mut()
            .from_key(&buffer)
            .or_insert_with(|| {
                (
                    ByPtr::clone(buffer),
                    ResourceInfo {
                        resource: None,
                        batch: u64::MAX,
                    },
                )
            })
            .1
    }

    pub(super) unsafe fn set_avail_batch(&mut self, avail_batch: u64) {
        self.avail_batch = avail_batch;
    }

    pub(crate) fn get_state(&self, image: &Arc<ImageDef>) -> ResourceState {
        tryopt!(self
            .images
            .get(ByPtr::by_ptr(image))?
            .state(self.avail_batch))
        .unwrap_or(ResourceState::Unavailable)
    }

    pub(crate) fn prepare_image_for_upload(
        &mut self,
        image: &Arc<ImageDef>,
        batch: u64,
        heap: &ImageHeap,
    ) -> &mut Arc<Image> {
        trace!(
            concat!(
                "ResourceStateTable::prepare_image_for_upload(",
                "image: {:?}, batch: {:?})",
            ),
            fmt_named(&**image),
            batch,
        );

        let info = self.get_or_init_image(image);
        info.batch = batch;
        info.resource
            .get_or_insert_with(|| Arc::new(Image::new(heap, Arc::clone(image))))
    }

    pub(crate) fn alloc_buffer(
        &mut self,
        def: &Arc<BufferDef>,
        heap: &Arc<BufferHeap>,
    ) -> &mut Arc<BufferAlloc> {
        let alloc = Arc::new(heap.alloc(def.binding, def.lifetime, def.mapping, def.size));
        let info = self.get_or_init_buffer(def);
        info.resource = Some(alloc);
        info.resource.as_mut().unwrap()
    }

    /// Makes a buffer immediately available.
    pub(crate) fn make_buffer_available(&mut self, def: &Arc<BufferDef>) {
        self.buffers.get_mut(ByPtr::by_ptr(def)).unwrap().batch = 0;
    }

    pub(crate) fn get_image(&self, image: &Arc<ImageDef>) -> Option<&Arc<Image>> {
        let info = self.images.get(ByPtr::by_ptr(image))?;
        guard(info.state(self.avail_batch) == ResourceState::Available)?;
        info.resource.as_ref()
    }

    pub(crate) fn get_buffer(&self, buffer: &Arc<BufferDef>) -> Option<&Arc<BufferAlloc>> {
        let info = self.buffers.get(ByPtr::by_ptr(buffer))?;
        guard(info.state(self.avail_batch) == ResourceState::Available)?;
        info.resource.as_ref()
    }

    #[allow(dead_code)]
    pub(crate) fn invalidate_image(&mut self, image: &Arc<ImageDef>) {
        self.images.remove(ByPtr::by_ptr(image));
    }
}
