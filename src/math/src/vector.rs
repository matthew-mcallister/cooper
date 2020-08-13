#![allow(clippy::too_many_arguments)]

use std::ops::*;

use base::impl_bin_ops;
use num::*;

use crate::{Cross, Dot};

/// A general-purpose fixed-size vector for fast calculations at
/// low dimensions.
// TODO: Explicit/guaranteed SIMD
// TODO: Aligned variants. But how to implement? A macro or wrappers?
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Vector<F, const N: usize> {
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

impl<F, const N: usize> Vector<F, N> {
    #[inline(always)]
    pub fn new(elems: [F; N]) -> Self {
        elems.into()
    }

    #[inline(always)]
    pub fn iter(&self) -> impl ExactSizeIterator<Item=&F> {
        self.elems.iter()
    }

    #[inline(always)]
    pub fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = &mut F> {
        self.elems.iter_mut()
    }
}

impl<F: Copy, const N: usize> Vector<F, N> {
    #[inline(always)]
    pub fn from_scalar(val: F) -> Self {
        [val; N].into()
    }
}

#[inline(always)]
pub fn vec<F, const N: usize>(elems: [F; N]) -> Vector<F, N> {
    Vector::new(elems)
}

macro_rules! impl_vecn {
    ($vecn:ident, $N:expr, $($arg:ident),*) => {
        #[inline(always)]
        pub fn $vecn<F>($($arg: F,)*) -> Vector<F, $N> {
            Vector::new([$($arg,)*])
        }

        impl<F> From<($(konst!({F}, $arg)),*)> for Vector<F, $N> {
            fn from(($($arg),*): ($(konst!({F}, $arg)),*)) -> Self {
                Vector::new([$($arg),*])
            }
        }

        impl<F> From<Vector<F, $N>> for ($(konst!({F}, $arg)),*) {
            fn from(Vector { elems: [$($arg),*] }: Vector<F, $N>) -> Self {
                ($($arg),*)
            }
        }
    }
}

impl_vecn!(vec2, 2, a, b);
impl_vecn!(vec3, 3, a, b, c);
impl_vecn!(vec4, 4, a, b, c, d);
impl_vecn!(vec5, 5, a, b, c, d, e);
impl_vecn!(vec6, 6, a, b, c, d, e, f);
impl_vecn!(vec7, 7, a, b, c, d, e, f, g);
impl_vecn!(vec8, 8, a, b, c, d, e, f, g, h);
impl_vecn!(vec9, 9, a, b, c, d, e, f, g, h, i);

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

impl<F: Copy> Vector<F, 1> { impl_accessors!(0); }
impl<F: Copy> Vector<F, 2> { impl_accessors!(0 1); }
impl<F: Copy> Vector<F, 3> { impl_accessors!(0 1 2); }
impl<F: Copy> Vector<F, 4> { impl_accessors!(0 1 2 3); }

impl<F: std::fmt::Debug, const N: usize> std::fmt::Debug for Vector<F, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.elems[..].fmt(f)
    }
}

impl<F, const N: usize> From<[F; N]> for Vector<F, N> {
    #[inline(always)]
    fn from(elems: [F; N]) -> Self {
        Vector { elems }
    }
}

impl<F, const N: usize> From<Vector<F, N>> for [F; N] {
    #[inline(always)]
    fn from(vec: Vector<F, N>) -> Self {
        vec.elems
    }
}

// TODO: impl [Try]From<&[F]>

// TODO: This impl should require neither E: Default + Copy nor F: Copy
impl<F: Copy, const N: usize> Vector<F, N> {
    #[inline(always)]
    pub fn map<E: Default + Copy>(self, mut f: impl FnMut(F) -> E) ->
        Vector<E, N>
    {
        let mut out = Vector::default();
        for (dst, &src) in out.iter_mut().zip(self.iter()) {
            *dst = f(src);
        }
        out
    }
}

// TODO: This impl should not require F: Copy
impl<F: Default + Copy, const N: usize> Default for Vector<F, N> {
    #[inline(always)]
    fn default() -> Self {
        [Default::default(); N].into()
    }
}

impl<F: PartialEq, const N: usize> PartialEq for Vector<F, N> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        PartialEq::eq(&self.elems[..], &other.elems[..])
    }
}

impl<F: Eq, const N: usize> Eq for Vector<F, N> {}

// TODO: More ops, e.g. Hash

// TODO: Maybe impl AsRef<[F]>, AsRef<[[F; N]]> for [Vector<F, N>]
impl<F, const N: usize> AsRef<[F; N]> for Vector<F, N> {
    #[inline(always)]
    fn as_ref(&self) -> &[F; N] {
        &self.elems
    }
}

impl<F, const N: usize> AsMut<[F; N]> for Vector<F, N> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [F; N] {
        &mut self.elems
    }
}

impl<F, const N: usize> AsRef<[F]> for Vector<F, N> {
    #[inline(always)]
    fn as_ref(&self) -> &[F] {
        &self.elems[..]
    }
}

impl<F, const N: usize> AsMut<[F]> for Vector<F, N> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [F] {
        &mut self.elems[..]
    }
}

derive_index!(
    (F, const N: usize),
    Vector<F, N>, elems, [F],
);

impl<F: Zero + Copy, const N: usize> Zero for Vector<F, N> {
    #[inline(always)]
    fn zero() -> Self {
        Self::from_scalar(Zero::zero())
    }
}

impl_un_op!({F: PrimSigned, const N: usize}, (Vector<F, N>), Neg, neg);
impl_un_op!({F: PrimInt, const N: usize}, (Vector<F, N>), Not, not);

impl_bin_op!(
    {F: Primitive, const N: usize}, (Vector<F, N>),
    Add, AddAssign, add, add_assign
);
impl_bin_op!(
    {F: Primitive, const N: usize}, (Vector<F, N>),
    Sub, SubAssign, sub, sub_assign
);
impl_bin_op!(
    {F: Primitive, const N: usize}, (Vector<F, N>),
    Mul, MulAssign, mul, mul_assign
);
impl_bin_op!(
    {F: Primitive, const N: usize}, (Vector<F, N>),
    Div, DivAssign, div, div_assign
);
impl_bin_op!(
    {F: Primitive, const N: usize}, (Vector<F, N>),
    Rem, RemAssign, rem, rem_assign
);

impl_scalar_op!(
    {F: Primitive, const N: usize}, (Vector<F, N>), (F),
    Mul, MulAssign, mul, mul_assign
);
impl_scalar_op!(
    {F: Primitive, const N: usize}, (Vector<F, N>), (F),
    Div, DivAssign, div, div_assign
);
impl_scalar_op!(
    {F: Primitive, const N: usize}, (Vector<F, N>), (F),
    Rem, RemAssign, rem, rem_assign
);

impl<F: Primitive, const N: usize> std::iter::Sum for Vector<F, N> {
    #[inline(always)]
    fn sum<I>(iter: I) -> Self
        where I: Iterator<Item = Self>
    {
        iter.fold(Default::default(), Add::add)
    }
}

// TODO: Bitwise ops (should work for a boolean vector as well)

macro_rules! impl_dot {
    ({$($lt:tt)*}, ($Lhs:ty), ($Rhs:ty)) => {
        impl<$($lt)* F: Primitive, const N: usize> Dot<$Rhs> for $Lhs {
            type Output = F;
            #[inline(always)]
            fn dot(self, rhs: $Rhs) -> Self::Output {
                self.iter().zip(rhs.iter()).map(|(l, r)| l * r).sum()
            }
        }
    }
}

impl_dot!({}, (Vector<F, N>), (Vector<F, N>));
impl_dot!({'rhs,}, (Vector<F, N>), (&'rhs Vector<F, N>));
impl_dot!({'lhs,}, (&'lhs Vector<F, N>), (Vector<F, N>));
impl_dot!({'lhs, 'rhs,}, (&'lhs Vector<F, N>), (&'rhs Vector<F, N>));

macro_rules! impl_cross {
    ({$($lt:tt)*}, ($Lhs:ty), ($Rhs:ty)) => {
        impl<$($lt)* F: Primitive> Cross<$Rhs> for $Lhs {
            type Output = Vector3<F>;
            fn cross(self, rhs: $Rhs) -> Self::Output {
                vec3(
                    self[1] * rhs[2] - self[2] * rhs[1],
                    self[2] * rhs[0] - self[0] * rhs[2],
                    self[0] * rhs[1] - self[1] * rhs[0],
                )
            }
        }
    }
}

impl_cross!({}, (Vector3<F>), (Vector3<F>));
impl_cross!({'rhs,}, (Vector3<F>), (&'rhs Vector3<F>));
impl_cross!({'lhs,}, (&'lhs Vector3<F>), (Vector3<F>));
impl_cross!({'lhs, 'rhs,}, (&'lhs Vector3<F>), (&'rhs Vector3<F>));

impl<F: Primitive + Signed + FloatOps, const N: usize> Vector<F, N> {
    pub fn length_sq(&self) -> F {
        self.dot(self)
    }

    pub fn length(&self) -> F {
        self.length_sq().sqrt()
    }

    pub fn normalized(self) -> Self {
        self / self.length()
    }

    pub fn normalize(&mut self) -> F {
        let length = self.length();
        *self /= length;
        length
    }
}

// TODO: More general swizzles might be desirable.

impl<F: Zero + Copy> Vector3<F> {
    #[inline(always)]
    pub fn xyz0(&self) -> Vector4<F> {
        vec4(self[0], self[1], self[2], F::zero())
    }
}

impl<F: One + Copy> Vector3<F> {
    #[inline(always)]
    pub fn xyz1(&self) -> Vector4<F> {
        vec4(self[0], self[1], self[2], F::one())
    }
}

impl<F: Copy> Vector4<F> {
    #[inline(always)]
    pub fn xyz(&self) -> Vector3<F> {
        vec3(self[0], self[1], self[2])
    }
}

#[cfg(test)]
mod tests {
    use crate::{dot, cross};
    use super::*;

    #[test]
    fn vec0() {
        let u: Vector<f32, 0> = Default::default();
        let v: Vector<f32, 0> = Vector::new([]);
        assert_eq!(u, v);
        assert_eq!(u + v, u - v);
    }

    #[test]
    fn vec1() {
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
    fn accessors() {
        let v: Vector3<f32> = Vector::new([1.0, 0.0, 0.0]);
        assert_eq!(v[0], 1.0);
        assert_eq!(v[1], 0.0);
        assert_eq!(v.x(), 1.0);
        assert_eq!(v.z(), 0.0);
        assert_eq!(v, [1.0, 0.0, 0.0].into());
        assert_eq!(&v[..], [1.0, 0.0, 0.0]);
        assert_eq!(v.as_ref(), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn vec_ops() {
        let v: Vector3<f32> = (1.0, 0.0, 0.0).into();
        let u: Vector3<f32> = vec([0.0, 1.0, 0.0]);
        assert_eq!(-v, vec3(-1.0, 0.0, 0.0));
        assert_eq!(u + v, vec3(1.0, 1.0, 0.0));
        assert_eq!(u - v, vec3(-1.0, 1.0, 0.0));
        assert_eq!(u * v, Zero::zero());
        assert_eq!(v - v, Zero::zero());
        assert_eq!(v + v, vec3(2.0, 0.0, 0.0));

        assert_eq!(vec2(2.0, 1.0) / vec2(1.0, 2.0), vec2(2.0, 0.5));
        assert_eq!(vec2(1.0, 1.0) % vec2(1.0, 2.0), vec2(0.0, 1.0));
    }

    #[test]
    fn scalar_ops() {
        let v: Vector3<f32> = Vector::new([1.0, 0.0, 0.0]);
        assert_eq!(v * 1.0, v);
        assert_eq!(v * 0.0, Zero::zero());
        assert_eq!(v * 2.0, v + v);
        assert_eq!(v * -1.0, -v);

        assert_eq!(v / 1.0, v);
        assert_eq!((v / 0.0)[0], f32::INFINITY);
        assert!((v / 0.0)[1].is_nan());
        assert_eq!(v / 2.0, v - v * 0.5);
        assert_eq!(v / -1.0, -v);

        assert_eq!(v % 1.0, Zero::zero());
        assert!((v % 0.0).iter().all(|x| x.is_nan()));
        assert_eq!(v % 2.0, v);
        assert_eq!(v % -1.0, Zero::zero());
    }

    #[test]
    fn dot_and_cross() {
        let b = [
            -vec3(0.0, 0.0, 1.0),
            -vec3(0.0, 1.0, 0.0),
            -vec3(1.0, 0.0, 0.0),
            vec3(0.0, 0.0, 0.0),
            vec3(1.0, 0.0, 0.0),
            vec3(0.0, 1.0, 0.0),
            vec3(0.0, 0.0, 1.0),
        ];
        let e = &b[4..7];
        let k = [
            [ 0,  3, -2],
            [-3,  0,  1],
            [ 2, -1,  0],
        ];
        for i in 0..3 {
            for j in 0..3 {
                assert_eq!(dot(e[i], e[j]), if i == j { 1.0 } else { 0.0 });
                assert_eq!(cross(e[i], e[j]), b[(k[i][j] + 3) as usize]);
            }
        }
        assert_eq!(dot(cross(e[0], e[1]), e[2]), 1.0);
        assert_eq!(dot(cross(e[0], e[1]), e[0]), 0.0);
    }

    #[test]
    fn methods() {
        assert_eq!(vec2(1.0, 0.0).length_sq(), 1.0);
        assert_eq!(vec2(3.0, 4.0).length(), 5.0);
        assert_eq!(vec2(2.0, 0.0).normalized(), vec2(1.0, 0.0));

        let mut x = vec2(2.0, 0.0);
        let len = x.normalize();
        assert_eq!(x, vec2(1.0, 0.0));
        assert_eq!(len, 2.0);

        assert_eq!(vec2(0i32, 1i32).map(|x| x as u32), vec2(0u32, 1u32));
    }

    #[test]
    fn swizzle() {
        let a = vec3(1.0, 2.0, 3.0);
        assert_eq!(a.xyz0(), vec4(1.0, 2.0, 3.0, 0.0));
        assert_eq!(a.xyz1(), vec4(1.0, 2.0, 3.0, 1.0));
        let b = vec4(1.0, 2.0, 3.0, 4.0);
        assert_eq!(b.xyz(), a);
    }
}
