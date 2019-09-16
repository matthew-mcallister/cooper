use std::cmp;
use std::ops::Deref;

use derive_more::*;

/// Wraps a smart pointer so the traits it implements operate on the
/// pointer itself rather than the referenced object.
///
/// Note that `ByPtr<P>` doesn't inherit `Borrow` impls from `P`, as it
/// specifically does not use the same trait impls (e.g. `Eq` and
/// `Hash`) as the referenced type. Sadly,
#[derive(Clone, Debug, Deref, DerefMut, From)]
pub struct ByPtr<P> {
    inner: P,
}

impl<P, T> AsRef<T> for ByPtr<P>
    where P: AsRef<T>
{
    #[inline(always)]
    fn as_ref(&self) -> &T {
        self.inner.as_ref()
    }
}

impl<P, T> AsMut<T> for ByPtr<P>
    where P: AsMut<T>
{
    #[inline(always)]
    fn as_mut(&mut self) -> &mut T {
        self.inner.as_mut()
    }
}

impl<P> PartialEq for ByPtr<P>
    where Self: Deref
{
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.as_ptr() == other.as_ptr()
    }
}

impl<P> Eq for ByPtr<P>
    where Self: Deref
{
}

impl<P> PartialOrd for ByPtr<P>
    where Self: Deref
{
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.as_ptr().partial_cmp(&other.as_ptr())
    }
}

impl<P> Ord for ByPtr<P>
    where Self: Deref
{
    #[inline(always)]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.as_ptr().cmp(&other.as_ptr())
    }
}

impl<P> std::hash::Hash for ByPtr<P>
    where Self: Deref
{
    #[inline(always)]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_ptr().hash(state)
    }
}

impl<P> ByPtr<P>
    where Self: Deref
{
    #[inline(always)]
    pub fn as_ptr(&self) -> *const <Self as Deref>::Target {
        self.deref() as _
    }
}

impl<P> ByPtr<P> {
    #[inline(always)]
    pub fn new(ptr: P) -> Self {
        ByPtr { inner: ptr }
    }

    #[inline(always)]
    pub fn into_inner(self) -> P {
        self.inner
    }

    #[inline(always)]
    pub fn from_ref(ptr: &P) -> &Self {
        unsafe { std::mem::transmute(ptr) }
    }

    #[inline(always)]
    pub fn into_ref(&self) -> &P {
        &self.inner
    }

    #[inline(always)]
    pub fn from_mut(ptr: &mut P) -> &mut Self {
        unsafe { std::mem::transmute(ptr) }
    }

    #[inline(always)]
    pub fn into_mut(&mut self) -> &mut P {
        &mut self.inner
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::rc::Rc;
    use super::*;

    #[test]
    fn test_hash() {
        let x = &ByPtr::new(Rc::new(0u32));
        let y = &ByPtr::clone(&x);
        assert_eq!(&x, &y);

        let z = &ByPtr::new(Rc::new(1u32));
        let w = &ByPtr::clone(&z);
        let set: HashSet<_> = vec![x, y, z].into_iter().cloned().collect();
        assert_eq!(set.len(), 2);
        assert!(set.contains(y));
        assert!(set.contains(w));
    }

    #[test]
    fn test_cast() {
        let x = Rc::new(0u32);
        let y: &ByPtr<_> = ByPtr::from_ref(&x);
        let z = &ByPtr::clone(ByPtr::from_ref(&x));
        assert_eq!(y, z);

        let y = Rc::clone(&x);
        let x: ByPtr<Rc<u32>> = x.into();
        let x: Rc<u32> = x.into_inner();
        assert!(Rc::ptr_eq(&x, &y));
    }
}
