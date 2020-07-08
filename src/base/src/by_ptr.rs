use std::cmp::Ordering;
use std::collections::hash_map::{HashMap, RawEntryMut};
use std::hash::{BuildHasher, Hash, Hasher};
use std::ops::{Deref, DerefMut};

use derive_more::{AsRef, AsMut, Constructor, Deref, DerefMut, From};

#[derive(AsRef, AsMut, Clone, Constructor, Copy, Debug, Deref, DerefMut, From)]
#[as_ref(forward)]
#[deref(forward)]
#[deref_mut(forward)]
#[repr(transparent)]
pub struct ByPtr<P> {
    inner: P,
}

macro_rules! delegate {
    ($trait:ident::$fn:ident -> $ret:ty) => {
        #[inline(always)]
        fn $fn(&self, other: &Self) -> $ret {
            $trait::$fn(
                &(self.deref() as *const _),
                &(other.deref() as *const _),
            )
        }
    }
}

// TODO: Implement for any RHS that derefs to P::Target
impl<P: Deref> PartialEq for ByPtr<P> {
    delegate!(PartialEq::eq -> bool);
    delegate!(PartialEq::ne -> bool);
}

impl<P: Deref> Eq for ByPtr<P> {}

impl<P: Deref> PartialOrd for ByPtr<P> {
    delegate!(PartialOrd::partial_cmp -> Option<Ordering>);
    delegate!(PartialOrd::lt -> bool);
    delegate!(PartialOrd::le -> bool);
    delegate!(PartialOrd::gt -> bool);
    delegate!(PartialOrd::ge -> bool);
}

impl<P: Deref> Ord for ByPtr<P> {
    delegate!(Ord::cmp -> Ordering);
}

impl<P: Deref> Hash for ByPtr<P> {
    #[inline(always)]
    fn hash<H>(&self, state: &mut H)
        where H: Hasher
    {
        ByPtr::as_ptr(self).hash(state);
    }
}

impl<P> ByPtr<P> {
    #[inline(always)]
    pub fn by_value(this: &Self) -> &P {
        unsafe { std::mem::transmute(this) }
    }

    #[inline(always)]
    pub fn by_value_mut(this: &mut Self) -> &mut P {
        unsafe { std::mem::transmute(this) }
    }

    #[inline(always)]
    pub fn by_ptr(ptr: &P) -> &Self {
        unsafe { std::mem::transmute(ptr) }
    }

    #[inline(always)]
    pub fn by_ptr_mut(ptr: &mut P) -> &mut Self {
        unsafe { std::mem::transmute(ptr) }
    }
}

impl<P: Deref> ByPtr<P> {
    #[inline(always)]
    pub fn as_ptr(this: &Self) -> *const P::Target {
        this.deref() as *const _
    }

    /// Sadly we cannot *quite* use Borrow to look up a HashMap entry by
    /// raw pointer, but we can circumvent this using the raw entry API.
    pub fn hash_by_ptr<V, S: BuildHasher>(
        ptr: *const P::Target,
        map: &HashMap<ByPtr<P>, V, S>,
    ) -> Option<(&P, &V)> {
        let mut hasher = map.hasher().build_hasher();
        ptr.hash(&mut hasher);
        let hash = hasher.finish();

        let eq = |key: &ByPtr<_>| ByPtr::as_ptr(key) == ptr;
        let (key, val) = map.raw_entry().from_hash(hash, eq)?;
        Some((ByPtr::by_value(key), val))
    }

    pub fn hash_by_ptr_mut<V, S: BuildHasher>(
        ptr: *const P::Target,
        map: &mut HashMap<ByPtr<P>, V, S>,
    ) -> Option<(&P, &mut V)> {
        let mut hasher = map.hasher().build_hasher();
        ptr.hash(&mut hasher);
        let hash = hasher.finish();

        let eq = |key: &ByPtr<_>| ByPtr::as_ptr(key) == ptr;
        let entry = map.raw_entry_mut().from_hash(hash, eq);
        match entry {
            RawEntryMut::Occupied(entry) => {
                let (key, val) = entry.into_key_value();
                Some((ByPtr::by_value(key), val))
            },
            _ => None,
        }
    }
}

impl<P: DerefMut> ByPtr<P> {
    #[inline(always)]
    pub fn as_mut_ptr(this: &mut Self) -> *mut P::Target {
        this.deref_mut() as *mut _
    }
}

impl<'a, P> From<&'a P> for &'a ByPtr<P> {
    #[inline(always)]
    fn from(by_val: &'a P) -> Self {
        ByPtr::by_ptr(by_val)
    }
}

impl<'a, P> From<&'a mut P> for &'a mut ByPtr<P> {
    #[inline(always)]
    fn from(by_val: &'a mut P) -> Self {
        ByPtr::by_ptr_mut(by_val)
    }
}

#[test]
fn convert() {
    let x = -1i32;
    let y = x;
    let mut p = &x;
    let mut q = &y;
    let mut b = ByPtr::new(p);

    assert_eq!(<ByPtr<&i32>>::from(p), b);
    assert_eq!(<&ByPtr<&i32>>::from(&p), &b);
    assert_eq!(<&mut ByPtr<&i32>>::from(&mut p), &mut b);

    assert_eq!(ByPtr::by_value(&b), &p);
    assert_eq!(ByPtr::by_value_mut(&mut b), &mut p);
    assert_eq!(&b, ByPtr::by_ptr(&p));
    assert_eq!(&mut b, ByPtr::by_ptr(&mut p));

    assert_eq!(ByPtr::by_value(&b), &q);
    assert_eq!(ByPtr::by_value_mut(&mut b), &mut q);
    assert_ne!(&b, ByPtr::by_ptr(&q));
    assert_ne!(&mut b, ByPtr::by_ptr(&mut q));
}

#[test]
fn ops() {
    use std::collections::HashSet;

    let x = -1i32;
    let y = x;
    let p = ByPtr::new(&x);
    let q = ByPtr::new(&x);
    let r = ByPtr::new(&y);

    assert_eq!(p, q);
    assert_ne!(p, r);

    assert!(!(p < q) & !(p > q));
    assert!((p < r) ^ (p > r));

    let mut set = HashSet::new();
    set.insert(p);
    set.insert(q);
    set.insert(r);

    assert_eq!(set.len(), 2);
    assert!(set.contains(&p));
    assert!(set.contains(&q));
    assert!(set.contains(&r));
}

#[test]
fn hash_by_ptr() {
    use std::sync::Arc;

    let x = Arc::new(1i32);
    let ptr = &*x as *const i32;

    let mut map: HashMap<ByPtr<Arc<i32>>, i32> = HashMap::new();
    map.insert(Arc::clone(&x).into(), 42i32);

    let (key, val) = ByPtr::hash_by_ptr(ptr, &map).unwrap();
    assert!(Arc::ptr_eq(key, &x));
    assert_eq!(*val, 42i32);

    let (_, val) = ByPtr::hash_by_ptr_mut(ptr, &mut map).unwrap();
    *val = 43i32;
    assert_eq!(map[ByPtr::by_ptr(&x)], 43i32);
}
