use std::sync::Arc;

// Dummy object purely used to get a reference count.
#[derive(Clone, Debug)]
pub struct Sentinel {
    inner: Arc<()>,
}

impl std::cmp::PartialEq for Sentinel {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl std::cmp::Eq for Sentinel {}

impl Sentinel {
    pub fn new() -> Self {
        Sentinel { inner: Arc::new(()) }
    }

    pub fn in_use(&self) -> bool {
        Arc::strong_count(&self.inner) > 1
    }
}
