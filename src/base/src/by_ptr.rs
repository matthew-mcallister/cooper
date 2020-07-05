use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
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
