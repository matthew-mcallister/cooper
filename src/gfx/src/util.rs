use math::{Matrix4, Matrix4x3};

crate type SmallVec<T, const N: usize> = smallvec::SmallVec<[T; N]>;

macro_rules! tryopt {
    ($($body:tt)*) => { (try { $($body)* }: Option<_>) };
}

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

#[inline(always)]
crate fn pack_xform(xform: Matrix4<f32>) -> Matrix4x3<f32> {
    xform.transpose().submatrix(0, 0)
}

macro_rules! set_name {
    ($($var:expr),*$(,)?) => {
        {
            $($var.set_name(stringify!($var));)*
        }
    }
}
