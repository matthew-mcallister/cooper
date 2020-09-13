use std::ops::*;

use derivative::Derivative;
use num::{One, Zero};
use packed_simd::{f32x2, f32x4, shuffle};

pub unsafe trait Scalar<const N: usize> {
    type Vector: Copy + Default;
    fn splat(self) -> Self::Vector;
}

pub type VectorInner<F, const N: usize> = <F as Scalar<N>>::Vector;

macro_rules! impl_scalar {
    ($(($scalar:tt, $n:tt, $vec:tt))*) => {
        $(
            unsafe impl Scalar<$n> for $scalar {
                type Vector = $vec;

                #[inline(always)]
                fn splat(self) -> Self::Vector {
                    $vec::splat(self)
                }
            }
        )*
    }
}

impl_scalar! {
    (f32, 2, f32x2) (f32, 3, f32x4) (f32, 4, f32x4)
}

/// A small, SIMD-backed mathematical vector.
///
/// Note that objects of this type may be padded for optimal
/// alignment---for example, `Vector<f32, 3>` is 16 bytes in size and
/// alignment. However, it is possible to load a vector from an
/// unaligned primitive array.
#[derive(Derivative)]
#[derivative(
    Clone(bound = ""),
    Copy(bound = ""),
    Debug(bound = "<F as Scalar<N>>::Vector: std::fmt::Debug"),
    Default(bound = ""),
)]
pub struct Vector<F: Scalar<N>, const N: usize>(<F as Scalar<N>>::Vector);

pub type Vector2<F> = Vector<F, 2>;
pub type Vector3<F> = Vector<F, 3>;
pub type Vector4<F> = Vector<F, 4>;

#[inline(always)]
pub fn vec<F: Scalar<N>, const N: usize>(elems: [F; N]) -> Vector<F, N>
    where [F; N]: Into<Vector<F, N>>
{
    elems.into()
}

macro_rules! impl_vecn {
    ($vecn:ident, $N:expr, $($arg:ident),*) => {
        #[inline(always)]
        pub fn $vecn<F: Scalar<$N>>($($arg: F,)*) -> Vector<F, $N>
            where [F; $N]: Into<Vector<F, $N>>
        {
            vec([$($arg,)*])
        }
    }
}

impl_vecn!(vec2, 2, a, b);
impl_vecn!(vec3, 3, a, b, c);
impl_vecn!(vec4, 4, a, b, c, d);

macro_rules! impl_accessor {
    ($f:tt 0) => { impl_accessor!(@impl $f 0; x r s); };
    ($f:tt 1) => { impl_accessor!(@impl $f 1; y g t); };
    ($f:tt 2) => { impl_accessor!(@impl $f 2; z b p); };
    ($f:tt 3) => { impl_accessor!(@impl $f 3; w a q); };
    (@impl $f:tt $pos:expr; $($acc:ident)*) => {
        #[inline(always)]
        $(pub fn $acc(self) -> $f {
            unsafe { self.0.extract_unchecked($pos) }
        })*
    };
}

macro_rules! impl_accessors {
    ($f:tt, $n:tt { $($i:tt)* }) => {
        impl Vector<$f, $n> {
            $(impl_accessor!($f $i);)*
        }
    }
}

impl_accessors!(f32, 2 { 0 1 });
impl_accessors!(f32, 3 { 0 1 2 });
impl_accessors!(f32, 4 { 0 1 2 3 });

const fn mask(n: usize) -> u8 {
    if n < 32 { !(!0u8 << n) } else { 0 }
}

impl<F: Scalar<N>, const N: usize> Vector<F, N> {
    #[inline(always)]
    pub fn splat(scalar: F) -> Self {
        Self(scalar.splat())
    }
}

impl<F: Scalar<N> + Zero, const N: usize> Zero for Vector<F, N> {
    #[inline(always)]
    fn zero() -> Self {
        Self::splat(Zero::zero())
    }
}

impl<F: Scalar<N> + One, const N: usize> One for Vector<F, N> {
    #[inline(always)]
    fn one() -> Self {
        Self::splat(One::one())
    }
}

macro_rules! impl_un_op {
    ($Op:ident, $op:ident) => {
        impl<F, const N: usize> $Op for Vector<F, N>
        where
            F: Scalar<N>,
            VectorInner<F, N>: $Op<Output = VectorInner<F, N>>,
        {
            type Output = Vector<F, N>;
            #[inline(always)]
            fn $op(self) -> Vector<F, N> {
                Vector($Op::$op(self.0))
            }
        }
    }
}

impl_un_op!(Neg, neg);

// TODO?: Impls taking references
macro_rules! impl_vec_op {
    ($Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident) => {
        impl<F, const N: usize> $Op<Vector<F, N>> for Vector<F, N>
        where
            F: Scalar<N>,
            VectorInner<F, N>: $Op<Output = VectorInner<F, N>>,
        {
            type Output = Vector<F, N>;
            #[inline(always)]
            fn $op(self, other: Vector<F, N>) -> Vector<F, N> {
                Vector($Op::$op(self.0, other.0))
            }
        }

        impl<F, const N: usize> $OpAssign<Vector<F, N>> for Vector<F, N>
        where
            F: Scalar<N>,
            VectorInner<F, N>: $Op<Output = VectorInner<F, N>>,
        {
            #[inline(always)]
            fn $op_assign(&mut self, other: Vector<F, N>) {
                *self = Vector($Op::$op(self.0, other.0));
            }
        }
    }
}

impl_vec_op!(Add, AddAssign, add, add_assign);
impl_vec_op!(Sub, SubAssign, sub, sub_assign);
impl_vec_op!(Mul, MulAssign, mul, mul_assign);
impl_vec_op!(Div, DivAssign, div, div_assign);

impl<F: Scalar<N>, const N: usize> std::iter::Sum for Vector<F, N>
    where Self: Add<Output = Self>
{
    #[inline(always)]
    fn sum<I>(iter: I) -> Self
        where I: Iterator<Item = Self>
    {
        iter.fold(Default::default(), Add::add)
    }
}

macro_rules! impl_scalar_op {
    ($Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident) => {
        impl<F: Scalar<N>, const N: usize> $Op<F> for Vector<F, N>
            where Self: $Op<Output = Self>
        {
            type Output = Vector<F, N>;
            #[inline(always)]
            fn $op(self, scalar: F) -> Vector<F, N> {
                $Op::$op(self, Vector::<F, N>::splat(scalar))
            }
        }

        impl<F: Scalar<N>, const N: usize> $OpAssign<F> for Vector<F, N>
            where Self: $OpAssign
        {
            #[inline(always)]
            fn $op_assign(&mut self, scalar: F) {
                $OpAssign::$op_assign(self, Vector::<F, N>::splat(scalar))
            }
        }
    }
}

impl_scalar_op!(Mul, MulAssign, mul, mul_assign);
impl_scalar_op!(Div, DivAssign, div, div_assign);

macro_rules! impl_scalar_op_reverse {
    ($f:tt, $n:tt, $Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident) =>
    {
        impl $Op<Vector<$f, $n>> for $f {
            type Output = Vector<$f, $n>;
            #[inline(always)]
            fn $op(self, vector: Vector<$f, $n>) -> Vector<$f, $n> {
                $Op::$op(Vector::<$f, $n>::splat(self), vector)
            }
        }
    }
}

// Impls that are the same for all versions
macro_rules! impl_common {
    ($f:tt, $n:tt, $inner:ty; $($x:ident)*) => {
        impl From<[$f; $n]> for Vector<$f, $n> {
            #[inline(always)]
            fn from([$($x),*]: [$f; $n]) -> Self {
                Self::new($($x),*)
            }
        }

        impl From<Vector<$f, $n>> for [$f; $n] {
            #[inline(always)]
            fn from(vec: Vector<$f, $n>) -> Self {
                [$(vec.$x()),*]
            }
        }

        impl AsRef<[$f; $n]> for Vector<$f, $n> {
            #[inline(always)]
            fn as_ref(&self) -> &[$f; $n] {
                unsafe { &*(self as *const Self as *const [$f; $n]) }
            }
        }

        impl AsMut<[$f; $n]> for Vector<$f, $n> {
            #[inline(always)]
            fn as_mut(&mut self) -> &mut [$f; $n] {
                unsafe { &mut *(self as *mut Self as *mut [$f; $n]) }
            }
        }

        impl PartialEq for Vector<$f, $n> {
            #[inline(always)]
            fn eq(&self, other: &Self) -> bool {
                const MASK: u8 = mask($n);
                (self.0.eq(other.0).bitmask() & MASK) == MASK
            }

            #[inline(always)]
            fn ne(&self, other: &Self) -> bool {
                const MASK: u8 = mask($n);
                (self.0.ne(other.0).bitmask() & MASK) == MASK
            }
        }

        impl Vector<$f, $n> {
            #[inline(always)]
            pub fn le(self, other: Self) -> bool {
                const MASK: u8 = mask($n);
                (self.0.le(other.0).bitmask() & MASK) == MASK
            }

            #[inline(always)]
            pub fn lt(self, other: Self) -> bool {
                const MASK: u8 = mask($n);
                (self.0.lt(other.0).bitmask() & MASK) == MASK
            }

            #[inline(always)]
            pub fn ge(self, other: Self) -> bool {
                const MASK: u8 = mask($n);
                (self.0.ge(other.0).bitmask() & MASK) == MASK
            }

            #[inline(always)]
            pub fn gt(self, other: Self) -> bool {
                const MASK: u8 = mask($n);
                (self.0.gt(other.0).bitmask() & MASK) == MASK
            }

            /// Returns the infimum of two vectors, which is the minimum
            /// of the two taken component-wise.
            #[inline(always)]
            pub fn inf(self, other: Self) -> Self {
                Self(self.0.min(other.0))
            }

            /// Returns the supremum of two vectors, which is the
            /// maximum of the two taken component-wise.
            #[inline(always)]
            pub fn sup(self, other: Self) -> Self {
                Self(self.0.max(other.0))
            }

            #[inline(always)]
            pub fn dot(self, other: Self) -> $f {
                (self * other).sum()
            }

            #[inline(always)]
            pub fn length_sq(self) -> $f {
                self.dot(self)
            }

            #[inline(always)]
            pub fn length(self) -> $f {
                self.length_sq().sqrt()
            }

            #[inline(always)]
            pub fn normalized(mut self) -> Self {
                self /= self.length();
                self
            }

            #[inline(always)]
            pub fn normalize(&mut self) -> $f {
                let length = self.length();
                *self /= length;
                length
            }
        }

        impl_inf_sup!(Vector<$f, $n>);
        impl_scalar_op_reverse!($f, $n, Mul, MulAssign, mul, mul_assign);
        impl_scalar_op_reverse!($f, $n, Div, DivAssign, div, div_assign);
    }
}

impl_common!(f32, 2, f32x2; x y);
impl_common!(f32, 3, f32x4; x y z);
impl_common!(f32, 4, f32x4; x y z w);

macro_rules! impl_po2 {
    ($f:tt, $n:tt, $inner:tt; $($x:ident)*) => {
        impl Vector<$f, $n> {
            #[inline(always)]
            pub fn new($($x: $f),*) -> Self {
                Self($inner::new($($x),*))
            }

            #[inline(always)]
            pub fn load(src: &[$f; $n]) -> Self {
                unsafe {
                    Self($inner::from_slice_unaligned_unchecked(&src[..]))
                }
            }

            #[inline(always)]
            pub fn store(self, dst: &mut [$f; $n]) {
                unsafe {
                    self.0.write_to_slice_unaligned_unchecked(&mut dst[..])
                }
            }

            #[inline(always)]
            pub fn sum(self) -> $f {
                self.0.sum()
            }
        }
    }
}

impl_po2!(f32, 2, f32x2; x y);
impl_po2!(f32, 4, f32x4; x y z w);

impl Vector3<f32> {
    #[inline(always)]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        // N.B. This probably doesn't generate the best code every time.
        Self(f32x4::new(x, y, z, z))
    }

    #[inline(always)]
    pub fn load(&[x, y, z]: &[f32; 3]) -> Self {
        Self::new(x, y, z)
    }

    #[inline(always)]
    pub fn store(self, array: &mut [f32; 3]) {
        *array = *self.as_ref();
    }

    #[inline(always)]
    pub fn sum(self) -> f32 {
        self.xyz0().sum()
    }

    #[inline(always)]
    pub fn cross(self, other: Self) -> Self {
        (self * other.yzx() - self.yzx() * other).yzx()
    }

    #[inline(always)]
    pub fn yzx(self) -> Self {
        Vector(shuffle!(self.0, self.0, [1, 2, 0, 3]))
    }

    #[inline(always)]
    pub fn zxy(self) -> Self {
        Vector(shuffle!(self.0, self.0, [2, 0, 1, 3]))
    }

    #[inline(always)]
    pub fn xyz0(self) -> Vector4<f32> {
        Vector(self.0.replace(3, 0.0))
    }

    #[inline(always)]
    pub fn xyz1(self) -> Vector4<f32> {
        Vector(self.0.replace(3, 1.0))
    }
}

impl Vector4<f32> {
    #[inline(always)]
    pub fn xyz(self) -> Vector3<f32> {
        Vector(self.0)
    }
}

#[cfg(test)]
mod tests {
    use crate::{InfSupResult, MathItertools};
    use super::*;

    #[test]
    fn bitmask() {
        assert_eq!(mask(0), 0b0000);
        assert_eq!(mask(1), 0b0001);
        assert_eq!(mask(2), 0b0011);
        assert_eq!(mask(3), 0b0111);
        assert_eq!(mask(4), 0b1111);
    }

    #[test]
    fn accessors() {
        let v = Vector3::new(1.0, 0.0, 0.0);
        assert_eq!(v.x(), 1.0);
        assert_eq!(v.z(), 0.0);
        assert_eq!(v, [1.0, 0.0, 0.0].into());
        assert_eq!(<[f32; 3]>::from(v), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn vec_ops() {
        let v: Vector3<f32> = [1.0, 0.0, 0.0].into();
        let u: Vector3<f32> = vec([0.0, 1.0, 0.0]);
        assert_eq!(-v, vec3(-1.0, 0.0, 0.0));
        assert_eq!(u + v, vec3(1.0, 1.0, 0.0));
        assert_eq!(u - v, vec3(-1.0, 1.0, 0.0));
        assert_eq!(u * v, Zero::zero());
        assert_eq!(v - v, Zero::zero());
        assert_eq!(v + v, vec3(2.0, 0.0, 0.0));
        assert_eq!(vec2(2.0, 1.0) / vec2(1.0, 2.0), vec2(2.0, 0.5));
    }

    #[test]
    fn scalar_ops() {
        let v = Vector3::new(1.0, 0.0, 0.0);
        assert_eq!(v * 1.0, v);
        assert_eq!(v * 0.0, Zero::zero());
        assert_eq!(v * 2.0, v + v);
        assert_eq!(v * -1.0, -v);

        assert_eq!(v / 1.0, v);
        assert_eq!((v / 0.0).x(), f32::INFINITY);
        assert!((v / 0.0).y().is_nan());
        assert_eq!(v / 2.0, v - v * 0.5);
        assert_eq!(v / -1.0, -v);
    }

    #[test]
    fn dot_and_cross() {
        assert_eq!(vec3(1.0, 2.0, -1.0).dot(vec3(2.0, -1.0, 1.0)), -1.0);
        assert_eq!(vec3(1.0, 2.0, -1.0).cross(vec3(2.0, -1.0, 1.0)),
            vec3(1.0, -3.0, -5.0));

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
                assert_eq!(e[i].dot(e[j]), if i == j { 1.0 } else { 0.0 });
                assert_eq!(e[i].cross(e[j]), b[(k[i][j] + 3) as usize]);
            }
        }
        assert_eq!(e[0].cross(e[1]).dot(e[2]), 1.0);
        assert_eq!(e[0].cross(e[1]).dot(e[0]), 0.0);
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
    }

    #[test]
    fn swizzle() {
        let a = vec3(1.0, 2.0, 3.0);
        assert_eq!(a.xyz0(), vec4(1.0, 2.0, 3.0, 0.0));
        assert_eq!(a.xyz1(), vec4(1.0, 2.0, 3.0, 1.0));
        assert_eq!(a.yzx(), vec3(2.0, 3.0, 1.0));
        assert_eq!(a.zxy(), vec3(3.0, 1.0, 2.0));
        let b = vec4(1.0, 2.0, 3.0, 4.0);
        assert_eq!(b.xyz(), a);
    }

    #[test]
    fn order() {
        assert!(vec2(0.0, 0.0).le(Zero::zero()));
        assert!(vec2(0.0, 0.0).ge(Zero::zero()));
        assert!(!vec2(0.0, 0.0).lt(Zero::zero()));
        assert!(!vec2(0.0, 0.0).gt(Zero::zero()));

        assert!(vec2(0.0, 1.0).lt(vec2(1.0, 2.0)));
        assert!(vec2(0.0, 1.0).le(vec2(1.0, 2.0)));
        assert!(!vec2(0.0, 1.0).gt(vec2(1.0, 2.0)));
        assert!(!vec2(0.0, 1.0).ge(vec2(1.0, 2.0)));
    }

    #[test]
    fn inf_sup() {
        assert_eq!(vec2(1.0, 0.0).inf(vec2(0.0, 1.0)), vec2(0.0, 0.0));
        assert_eq!(vec2(0.0, 1.0).inf(vec2(1.0, 0.0)), vec2(0.0, 0.0));
        assert_eq!(vec2(1.0, 0.0).sup(vec2(0.0, 1.0)), vec2(1.0, 1.0));
        assert_eq!(vec2(0.0, 1.0).sup(vec2(1.0, 0.0)), vec2(1.0, 1.0));

        let vecs = &[
            vec2( 0.0, 0.0),
            vec2(-1.0, 3.0),
            vec2( 0.0, 1.0),
            vec2( 2.0, 2.0),
        ];
        let inf = vec2(-1.0, 0.0);
        let sup = vec2(2.0, 3.0);
        assert_eq!(vecs.iter().copied().inf(), Some(inf));
        assert_eq!(vecs.iter().copied().sup(), Some(sup));
        assert_eq!(vecs.iter().copied().inf_sup(),
            InfSupResult::InfSup(inf, sup));

        assert_eq!([].iter().cloned().inf_sup::<Vector2<f32>>(),
            InfSupResult::Empty);
        let vecs = &[vec2(1.0, 1.0)];
        assert_eq!(vecs.iter().cloned().inf_sup(),
            InfSupResult::Singleton(vec2(1.0, 1.0)));
    }
}
