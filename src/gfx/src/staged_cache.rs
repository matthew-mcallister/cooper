use std::borrow::Cow;
use std::fmt::Debug;
use std::hash::Hash;

use derivative::Derivative;
use fnv::FnvHashMap;
use parking_lot::Mutex;

/// Implements the caching logic used by pipelines and samplers.
/// Committed cache hits require no synchronization, which is ideal for
/// objects which are often used but rarely created.
// TODO: This doesn't actually allow *parallel* object creation due to
// the lock. The staging area could be stratified somewhat. Or async
// programming could be employed.
// TODO: Parameterize over hash function: FNV is a poor choice for pipes
#[derive(Derivative)]
#[derivative(Debug(bound = "FnvHashMap<K, V>: Debug"))]
crate struct StagedCache<K, V> {
    committed: FnvHashMap<K, V>,
    // TODO: *Maybe* should be a true concurrent hashmap
    staged: Mutex<FnvHashMap<K, V>>,
}

impl<K: Eq + Hash, V> Default for StagedCache<K, V> {
    fn default() -> Self {
        StagedCache {
            committed: Default::default(),
            staged: Default::default(),
        }
    }
}

impl<K, V> StagedCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    crate fn new() -> Self {
        Default::default()
    }

    /// Gets a committed entry with zero synchronization guaranteed.
    crate fn get_committed(&self, key: &K) -> Option<&V> {
        self.committed.get(key)
    }

    /// Commits all staged additions.
    crate fn commit(&mut self) {
        self.committed.extend(std::mem::take(self.staged.get_mut()));
    }

    // TODO: Allow f fallible.
    crate fn get_or_insert_with(
        &self,
        key: &K,
        f: impl FnOnce() -> V,
    ) -> Cow<V> {
        tryopt! { return Cow::Borrowed(self.get_committed(key)?); };
        let mut staged = self.staged.lock();
        // NB: hold the lock while creating entry to avoid racing
        let val = staged.entry(key.clone()).or_insert_with(f);
        Cow::Owned(val.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering::Relaxed;
    use super::*;

    // TODO: Doesn't require device
    unsafe fn smoke_test(_: crate::testing::TestVars) {
        let new = || Arc::new(AtomicUsize::new(0));

        let mut cache = StagedCache::new();
        let i = cache.get_or_insert_with(&-12, new);
        i.fetch_add(1, Relaxed);
        assert_eq!(i.load(Relaxed), 1);
        assert!(i.is_owned());

        cache.commit();

        cache.get_or_insert_with(&0, new);
        assert!(cache.get_committed(&0).is_none());
        assert!(cache.get_or_insert_with(&0, new).is_owned());

        assert_eq!(cache.get_committed(&-12).unwrap().load(Relaxed), 1);
        assert!(cache.get_or_insert_with(&-12, new).is_borrowed());
    }

    unit::declare_tests![smoke_test];
}

unit::collect_tests![tests];
