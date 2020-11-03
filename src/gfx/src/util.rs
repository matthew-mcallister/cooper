crate type SmallVec<T, const N: usize> = smallvec::SmallVec<[T; N]>;

macro_rules! impl_from_via_default {
    ($name:ident, $from:ty) => {
        impl From<$from> for $name {
            fn from(_: $from) -> Self {
                Default::default()
            }
        }
    }
}

#[repr(C)]
crate struct Aligned<T, U: ?Sized>(crate [T; 0], crate U);

impl<T> Aligned<T, [u8]> {
    const unsafe fn cast(&self) -> &[T] {
        let bytes = &self.1;
        let ptr = bytes as *const [u8] as *const u8 as *const T;
        let size_of = std::mem::size_of::<T>();
        assert!(bytes.len() % size_of == 0);
        let len = bytes.len() / size_of;
        &*std::ptr::slice_from_raw_parts(ptr, len)
    }
}

crate const fn cast_aligned_u32(aligned: &Aligned<u32, [u8]>) -> &[u32] {
    unsafe { aligned.cast() }
}

macro_rules! include_u32 {
    ($($source:tt)*) => {
        {
            static ALIGNED: &'static $crate::util::Aligned<u32, [u8]> =
                &$crate::util::Aligned([], *include_bytes!($($source)*));
            $crate::util::cast_aligned_u32(ALIGNED)
        }
    }
}

macro_rules! set_name {
    ($($var:expr),*$(,)?) => {
        {
            $($var.set_name(stringify!($var));)*
        }
    }
}

#[inline(always)]
crate fn ptr_eq<P, T>(lhs: &P, rhs: &P) -> bool
    where P: std::ops::Deref<Target = T>
{
    lhs.deref() as *const _ == rhs.deref() as *const _
}

#[inline(always)]
crate fn ptr_hash<P, H>(ptr: &P, state: &mut H)
where
    P: std::ops::Deref,
    H: std::hash::Hasher,
{
    std::hash::Hash::hash(&(ptr.deref() as *const P::Target), state)
}
