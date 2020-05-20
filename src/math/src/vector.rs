use std::ops::*;

use base::impl_bin_ops;

use prelude::num::*;

/// A general-purpose fixed-size vector for fast calculations at
/// low dimensions.
// TODO: Primitive is actually too strict---what about bvec?
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Vector<F: Primitive, const N: usize> {
    elems: [F; N],
}

pub type Vector2<F> = Vector<F, 2>;
pub type Vector3<F> = Vector<F, 3>;
pub type Vector4<F> = Vector<F, 4>;
pub type Vector5<F> = Vector<F, 5>;
pub type Vector6<F> = Vector<F, 6>;
pub type Vector7<F> = Vector<F, 7>;
pub type Vector8<F> = Vector<F, 8>;
pub type Vector9<F> = Vector<F, 9>;

impl<F: Primitive, const N: usize> Vector<F, N> {
    #[inline(always)]
    pub fn new(elems: [F; N]) -> Self {
        elems.into()
    }

    #[inline(always)]
    pub fn from_scalar(val: F) -> Self {
        [val; N].into()
    }

    #[inline(always)]
    pub fn iter(&self) -> impl Iterator<Item=&F> + ExactSizeIterator {
        self.elems.iter()
    }

    #[inline(always)]
    pub fn iter_mut(&mut self) ->
        impl Iterator<Item = &mut F> + ExactSizeIterator
    {
        self.elems.iter_mut()
    }
}

#[inline(always)]
pub fn vec<F: Primitive, const N: usize>(elems: [F; N]) -> Vector<F, N> {
    Vector::new(elems)
}

macro_rules! impl_vecn {
    ($VectorN:ident, $vecn:ident, $($arg:ident),*) => {
        #[inline(always)]
        pub fn $vecn<F: Primitive>($($arg: F,)*) -> $VectorN<F> {
            Vector::new([$($arg,)*])
        }
    }
}

impl_vecn!(Vector2, vec2, a, b);
impl_vecn!(Vector3, vec3, a, b, c);
impl_vecn!(Vector4, vec4, a, b, c, d);
impl_vecn!(Vector5, vec5, a, b, c, d, e);
impl_vecn!(Vector6, vec6, a, b, c, d, e, f);
impl_vecn!(Vector7, vec7, a, b, c, d, e, f, g);
impl_vecn!(Vector8, vec8, a, b, c, d, e, f, g, h);
impl_vecn!(Vector9, vec9, a, b, c, d, e, f, g, h, i);

macro_rules! impl_accessors {
    ($($n:tt)*) => { $(impl_accessor!($n);)* };
}

macro_rules! impl_accessor {
    (0) => { impl_accessor!(@impl 0; x r s); };
    (1) => { impl_accessor!(@impl 1; y g t); };
    (2) => { impl_accessor!(@impl 2; z b p); };
    (3) => { impl_accessor!(@impl 3; w a q); };
    (@impl $pos:expr; $($acc:ident)*) => {
        #[inline(always)]
        $(pub fn $acc(&self) -> F { self.elems[$pos] })*
    };
}

impl<F: Primitive> Vector<F, 1> { impl_accessors!(0); }
impl<F: Primitive> Vector<F, 2> { impl_accessors!(0 1); }
impl<F: Primitive> Vector<F, 3> { impl_accessors!(0 1 2); }
impl<F: Primitive> Vector<F, 4> { impl_accessors!(0 1 2 3); }

impl<F: Primitive, const N: usize> std::fmt::Debug for Vector<F, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.elems[..].fmt(f)
    }
}

impl<F: Primitive, const N: usize> From<[F; N]> for Vector<F, N> {
    #[inline(always)]
    fn from(elems: [F; N]) -> Self {
        Vector { elems }
    }
}

impl<F: Primitive, const N: usize> From<Vector<F, N>> for [F; N] {
    #[inline(always)]
    fn from(vec: Vector<F, N>) -> Self {
        vec.elems
    }
}

impl<F: Primitive, const N: usize> Default for Vector<F, N> {
    #[inline(always)]
    fn default() -> Self {
        [Default::default(); N].into()
    }
}

impl<F: Primitive, const N: usize> AsRef<[F; N]> for Vector<F, N> {
    #[inline(always)]
    fn as_ref(&self) -> &[F; N] {
        &self.elems
    }
}

impl<F: Primitive, const N: usize> AsRef<[F]> for Vector<F, N> {
    #[inline(always)]
    fn as_ref(&self) -> &[F] {
        &self.elems[..]
    }
}

macro_rules! impl_index {
    ($Output:ty, $idx:ty) => {
        impl<F: Primitive, const N: usize> Index<$idx> for Vector<F, N> {
            type Output = $Output;
            #[inline(always)]
            fn index(&self, idx: $idx) -> &Self::Output {
                &self.elems[idx]
            }
        }

        impl<F: Primitive, const N: usize> IndexMut<$idx> for Vector<F, N> {
            #[inline(always)]
            fn index_mut(&mut self, idx: $idx) -> &mut Self::Output {
                &mut self.elems[idx]
            }
        }
    }
}

// TODO: Maybe implement for Range etc.
impl_index!(F, usize);
impl_index!([F], RangeFull);

impl<F: Primitive, const N: usize> Zero for Vector<F, N> {
    #[inline(always)]
    fn zero() -> Self {
        Self::from_scalar(Zero::zero())
    }
}

macro_rules! impl_un_op {
    ($Bound:ident, $Op:ident, $op:ident) => {
        impl<F: $Bound, const N: usize> $Op for Vector<F, N> {
            type Output = Vector<F, N>;
            #[inline(always)]
            fn $op(mut self) -> Self::Output {
                for x in self.iter_mut() {
                    *x = $Op::$op(&*x);
                }
                self
            }
        }

        impl<'a, F: $Bound, const N: usize> $Op for &'a Vector<F, N> {
            type Output = Vector<F, N>;
            #[inline(always)]
            fn $op(self) -> Self::Output {
                let mut res = Self::Output::default();
                for (dst, &src) in res.iter_mut().zip(self.iter()) {
                    *dst = $Op::$op(src);
                }
                res
            }
        }
    }
}

pub trait PrimSigned = Primitive + Signed;

impl_un_op!(PrimSigned, Neg, neg);
impl_un_op!(PrimInt, Not, not);

// TODO: Bitwise ops (require a different trait bound)
macro_rules! impl_bin_op {
    ($Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident) => {
        impl<F: Primitive, const N: usize> $OpAssign<F>
            for Vector<F, N>
        {
            #[inline(always)]
            fn $op_assign(&mut self, rhs: F) {
                for dst in self.iter_mut() {
                    dst.$op_assign(rhs);
                }
            }
        }

        impl<'rhs, F: Primitive, const N: usize> $OpAssign<&'rhs F>
            for Vector<F, N>
        {
            #[inline(always)]
            fn $op_assign(&mut self, rhs: &'rhs F) {
                for dst in self.iter_mut() {
                    dst.$op_assign(rhs);
                }
            }
        }

        impl_bin_ops!(
            {F: Primitive, const N: usize},
            (Vector<F, N>), (F),
            copy,
            (std::ops::$Op), (std::ops::$OpAssign), $op, $op_assign,
        );

        impl<F: Primitive, const N: usize> $OpAssign for Vector<F, N> {
            #[inline(always)]
            fn $op_assign(&mut self, other: Self) {
                for (dst, src) in self.iter_mut().zip(other.iter()) {
                    dst.$op_assign(src);
                }
            }
        }

        impl<'rhs, F: Primitive, const N: usize> $OpAssign<&'rhs Self>
            for Vector<F, N>
        {
            #[inline(always)]
            fn $op_assign(&mut self, other: &'rhs Self) {
                for (dst, src) in self.iter_mut().zip(other.iter()) {
                    dst.$op_assign(src);
                }
            }
        }

        impl_bin_ops!(
            {F: Primitive, const N: usize},
            (Vector<F, N>), (Vector<F, N>),
            copy,
            (std::ops::$Op), (std::ops::$OpAssign), $op, $op_assign,
        );
    }
}

impl_bin_op!(Add, AddAssign, add, add_assign);
impl_bin_op!(Sub, SubAssign, sub, sub_assign);
impl_bin_op!(Mul, MulAssign, mul, mul_assign);
impl_bin_op!(Div, DivAssign, div, div_assign);
impl_bin_op!(Rem, RemAssign, rem, rem_assign);

// TODO: Bitwise ops

impl<F: Primitive, const N: usize> PartialEq for Vector<F, N> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        PartialEq::eq(&self.elems[..], &other.elems[..])
    }

#[inline(always)]
    fn ne(&self, other: &Self) -> bool {
        PartialEq::ne(&self.elems[..], &other.elems[..])
    }
}

impl<F: Primitive + Eq, const N: usize> Eq for Vector<F, N> {}

// TODO: More ops, e.g. Hash

impl<F: Primitive, const N: usize> std::iter::Sum for Vector<F, N> {
    #[inline(always)]
    fn sum<I>(iter: I) -> Self
        where I: Iterator<Item = Self>
    {
        iter.fold(Default::default(), Add::add)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec0_test() {
        let u: Vector<f32, 0> = Default::default();
        let v: Vector<f32, 0> = Vector::new([]);
        assert_eq!(u, v);
        assert_eq!(u + v, u - v);
    }

    #[test]
    fn vec1_test() {
        let u: Vector<f32, 1> = Default::default();
        let v = vec([1.0]);
        assert_eq!(u.x(), 0.0);
        assert_eq!(v[0], 1.0);
        assert_eq!(&v[..], [1.0]);
        assert_eq!(u + v, v);
        assert_eq!(u - v, -v);
        assert_eq!(u * v, u);
        assert_eq!(u / v, u);
        assert_eq!(u % v, u);
        assert_eq!(v / u, vec([f32::INFINITY]));
    }

    #[test]
    fn ops_test() {
        let v: Vector3<f32> = Vector::new([1.0, 0.0, 0.0]);
        assert_eq!(v[0], 1.0);
        assert_eq!(v[1], 0.0);
        assert_eq!(v.x(), 1.0);
        assert_eq!(v.z(), 0.0);
        assert_eq!(v, [1.0, 0.0, 0.0].into());
        assert_eq!(&v[..], [1.0, 0.0, 0.0]);
        assert_eq!(v.as_ref(), [1.0, 0.0, 0.0]);
        assert_eq!(-v, vec3(-1.0, 0.0, 0.0));

        let u: Vector3<f32> = vec([0.0, 1.0, 0.0]);
        assert_eq!(u + v, vec3(1.0, 1.0, 0.0));
        assert_eq!(u - v, vec3(-1.0, 1.0, 0.0));
        assert_eq!(u * v, Zero::zero());

        assert_eq!(vec2(2.0, 1.0) / vec2(1.0, 2.0), vec2(2.0, 0.5));
        assert_eq!(vec2(1.0, 1.0) % vec2(1.0, 2.0), vec2(0.0, 1.0));
    }
}
