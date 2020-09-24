use std::fmt::Debug;
use std::ops::*;

use derivative::Derivative;
use num::{One, Zero};
use packed_simd::{f32x2, f32x4, shuffle};

pub unsafe trait Scalar<const N: usize>: Copy + Default + Debug + One + Zero {
    type Vector: RawVector<N, Scalar = Self>;
}

pub unsafe trait RawVector<const N: usize>: Copy + Debug + Default {
    type Scalar: Scalar<N, Vector = Self>;
    fn splat(scalar: Self::Scalar) -> Self;
    fn load(src: &[Self::Scalar; N]) -> Self;
    fn store(self, dst: &mut [Self::Scalar; N]);
    fn eq(self, other: Self) -> bool;
    fn ne(self, other: Self) -> bool;
    fn sum(self) -> Self::Scalar;
}

pub trait VectorOps<F>
    = Sized
    + Neg<Output = Self>
    + Add<Self, Output = Self>
    + AddAssign<Self>
    + Sub<Self, Output = Self>
    + SubAssign<Self>
    + Mul<Self, Output = Self>
    + MulAssign<Self>
    + Mul<F, Output = Self>
    + MulAssign<F>
    + Div<Self, Output = Self>
    + DivAssign<Self>
    + Div<F, Output = Self>
    + DivAssign<F>
    ;

// TODO: x(), y(), z(), etc
pub trait Swizzle3<const N: usize>: Scalar<3> + Scalar<4> + Scalar<N> {
    fn xyz(v: Vector<Self, N>) -> Vector3<Self>;
    fn yzx(v: Vector<Self, N>) -> Vector3<Self>;
    fn zxy(v: Vector<Self, N>) -> Vector3<Self>;
    fn xyz_(v: Vector<Self, N>) -> Vector4<Self>;
    fn xyz0(v: Vector<Self, N>) -> Vector4<Self>;
    fn xyz1(v: Vector<Self, N>) -> Vector4<Self>;
}

pub trait GeneralScalar<const N: usize> = where
    Self: Scalar<N>,
    Vector<Self, N>: VectorOps<Self>,
    ;

pub trait GeneralFloat<const N: usize> = GeneralScalar<N> + num::Float;

pub trait BasicScalar
    = GeneralScalar<2>
    + GeneralScalar<3>
    + GeneralScalar<4>
    + Swizzle3<3>
    + Swizzle3<4>
    ;

pub trait BasicFloat = BasicScalar + num::Float;

type VectorInner<F, const N: usize> = <F as Scalar<N>>::Vector;

const fn mask(n: usize) -> u8 {
    if n < 32 { !(!0u8 << n) } else { 0 }
}

macro_rules! impl_scalar_pot {
    ($scalar:tt, $n:tt, $vec:tt; $($x:ident)*) => {
        unsafe impl Scalar<$n> for $scalar {
            type Vector = $vec;
        }

        unsafe impl RawVector<$n> for $vec {
            type Scalar = $scalar;

            #[inline(always)]
            fn splat(scalar: $scalar) -> Self {
                Self::splat(scalar)
            }

            #[inline(always)]
            fn load(src: &[$scalar; $n]) -> Self {
                unsafe {
                    $vec::from_slice_unaligned_unchecked(&src[..])
                }
            }

            #[inline(always)]
            fn store(self, dst: &mut [$scalar; $n]) {
                unsafe {
                    self.write_to_slice_unaligned_unchecked(&mut dst[..])
                }
            }

            #[inline(always)]
            fn eq(self, other: Self) -> bool {
                const MASK: u8 = mask($n);
                ($vec::eq(self, other).bitmask() & MASK) == MASK
            }

            #[inline(always)]
            fn ne(self, other: Self) -> bool {
                const MASK: u8 = mask($n);
                ($vec::ne(self, other).bitmask() & MASK) == MASK
            }

            #[inline(always)]
            fn sum(self) -> $scalar {
                $vec::sum(self)
            }
        }
    }
}

impl_scalar_pot!(f32, 2, f32x2; x y);
impl_scalar_pot!(f32, 4, f32x4; x y z w);

unsafe impl Scalar<3> for f32 {
    type Vector = f32x4;
}

unsafe impl RawVector<3> for f32x4 {
    type Scalar = f32;

    #[inline(always)]
    fn splat(scalar: Self::Scalar) -> Self {
        Self::splat(scalar)
    }

    #[inline(always)]
    fn load(&[x, y, z]: &[f32; 3]) -> Self {
        Self::new(x, y, z, z)
    }

    #[inline(always)]
    fn store(self, array: &mut [f32; 3]) {
        unsafe { *array = *(&self as *const  _ as *const [f32; 3]); }
    }

    #[inline(always)]
    fn eq(self, other: Self) -> bool {
        const MASK: u8 = mask(3);
        (f32x4::eq(self, other).bitmask() & MASK) == MASK
    }

    #[inline(always)]
    fn ne(self, other: Self) -> bool {
        const MASK: u8 = mask(3);
        (f32x4::ne(self, other).bitmask() & MASK) == MASK
    }

    #[inline(always)]
    fn sum(self) -> f32 {
        self.extract(0)
            + self.extract(1)
            + self.extract(2)
    }
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
    Debug(bound = ""),
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
        $(
            #[inline(always)]
            pub fn $acc(self) -> $f {
                unsafe { self.0.extract_unchecked($pos) }
            }
        )*
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

impl<F: Scalar<N>, const N: usize> Vector<F, N> {
    #[inline(always)]
    pub fn splat(scalar: F) -> Self {
        Self(F::Vector::splat(scalar))
    }

    #[inline(always)]
    pub fn load(src: &[F; N]) -> Self {
        Self(F::Vector::load(src))
    }

    #[inline(always)]
    pub fn store(self, dst: &mut [F; N]) {
        self.0.store(dst)
    }

    #[inline(always)]
    pub fn sum(self) -> F {
        self.0.sum()
    }

    #[inline(always)]
    pub fn dot(self, other: Self) -> F
        where Self: Mul<Output = Self>
    {
        (self * other).sum()
    }
}

impl<F: Scalar<N>, const N: usize> PartialEq for Vector<F, N> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(other.0)
    }

    #[inline(always)]
    fn ne(&self, other: &Self) -> bool {
        self.0.ne(other.0)
    }
}

impl<F: Scalar<N>, const N: usize> Zero for Vector<F, N> {
    #[inline(always)]
    fn zero() -> Self {
        Self::splat(Zero::zero())
    }
}

impl<F: Scalar<N>, const N: usize> One for Vector<F, N> {
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
            VectorInner<F, N>: $OpAssign,
        {
            #[inline(always)]
            fn $op_assign(&mut self, other: Vector<F, N>) {
                $OpAssign::$op_assign(&mut self.0, other.0);
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
                $Op::$op(vector, self)
            }
        }
    }
}

impl<F: Scalar<N>, const N: usize> From<[F; N]> for Vector<F, N> {
    #[inline(always)]
    fn from(array: [F; N]) -> Self {
        let mut vec = Self::default();
        unsafe { *(&mut vec as *mut _ as *mut [F; N]) = array; }
        vec
    }
}

impl<F: Scalar<N>, const N: usize> From<Vector<F, N>> for [F; N] {
    #[inline(always)]
    fn from(vec: Vector<F, N>) -> Self {
        unsafe { *(&vec as *const _ as *const [F; N]) }
    }
}

impl<F: Scalar<N>, const N: usize> AsRef<[F; N]> for Vector<F, N> {
    #[inline(always)]
    fn as_ref(&self) -> &[F; N] {
        unsafe { &*(self as *const Self as *const [F; N]) }
    }
}

impl<F: Scalar<N>, const N: usize> AsMut<[F; N]> for Vector<F, N> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [F; N] {
        unsafe { &mut *(self as *mut Self as *mut [F; N]) }
    }
}

impl<I, F: Scalar<N>, const N: usize> Index<I> for Vector<F, N>
    where [F]: Index<I>,
{
    type Output = <[F] as Index<I>>::Output;

    #[inline(always)]
    fn index(&self, idx: I) -> &Self::Output {
        self.as_ref().index(idx)
    }
}

impl<I, F: Scalar<N>, const N: usize> IndexMut<I> for Vector<F, N>
    where [F]: IndexMut<I>,
{
    #[inline(always)]
    fn index_mut(&mut self, idx: I) -> &mut Self::Output {
        self.as_mut().index_mut(idx)
    }
}

impl<F: Swizzle3<N>, const N: usize> Vector<F, N> {
    #[inline(always)]
    pub fn xyz(self) -> Vector3<F> {
        F::xyz(self)
    }

    #[inline(always)]
    pub fn yzx(self) -> Vector3<F> {
        F::yzx(self)
    }

    #[inline(always)]
    pub fn zxy(self) -> Vector3<F> {
        F::zxy(self)
    }

    #[inline(always)]
    pub fn xyz_(self) -> Vector4<F> {
        F::xyz_(self)
    }

    #[inline(always)]
    pub fn xyz0(self) -> Vector4<F> {
        F::xyz0(self)
    }

    #[inline(always)]
    pub fn xyz1(self) -> Vector4<F> {
        F::xyz1(self)
    }
}

impl Swizzle3<3> for f32 {
    #[inline(always)]
    fn xyz(v: Vector3<Self>) -> Vector3<Self> {
        v
    }

    #[inline(always)]
    fn yzx(v: Vector3<Self>) -> Vector3<Self> {
        Vector(shuffle!(v.0, v.0, [1, 2, 0, 3]))
    }

    #[inline(always)]
    fn zxy(v: Vector3<Self>) -> Vector3<Self> {
        Vector(shuffle!(v.0, v.0, [2, 0, 1, 3]))
    }

    #[inline(always)]
    fn xyz_(v: Vector3<Self>) -> Vector4<Self> {
        Vector(v.0)
    }

    #[inline(always)]
    fn xyz0(v: Vector3<Self>) -> Vector4<Self> {
        Vector(v.0.replace(3, 0.0))
    }

    #[inline(always)]
    fn xyz1(v: Vector3<Self>) -> Vector4<Self> {
        Vector(v.0.replace(3, 1.0))
    }
}

impl Swizzle3<4> for f32 {
    #[inline(always)]
    fn xyz(v: Vector4<Self>) -> Vector3<Self> {
        Vector(v.0)
    }

    #[inline(always)]
    fn yzx(v: Vector4<Self>) -> Vector3<Self> {
        v.xyz().yzx()
    }

    #[inline(always)]
    fn zxy(v: Vector4<Self>) -> Vector3<Self> {
        v.xyz().zxy()
    }

    #[inline(always)]
    fn xyz_(v: Vector4<Self>) -> Vector4<Self> {
        v
    }

    #[inline(always)]
    fn xyz0(v: Vector4<Self>) -> Vector4<Self> {
        Vector(v.0.replace(3, 0.0))
    }

    #[inline(always)]
    fn xyz1(v: Vector4<Self>) -> Vector4<Self> {
        Vector(v.0.replace(3, 1.0))
    }
}

impl<F: GeneralFloat<N>, const N: usize> Vector<F, N> {
    #[inline(always)]
    pub fn length_sq(self) -> F {
        self.dot(self)
    }

    #[inline(always)]
    pub fn length(self) -> F {
        self.length_sq().sqrt()
    }

    #[inline(always)]
    pub fn normalized(mut self) -> Self {
        self /= self.length();
        self
    }

    #[inline(always)]
    pub fn normalize(&mut self) -> F {
        let length = self.length();
        *self /= length;
        length
    }
}

impl<F: BasicScalar> Vector3<F> {
    #[inline(always)]
    pub fn cross(self, other: Self) -> Self {
        (self * other.yzx() - self.yzx() * other).yzx()
    }
}

macro_rules! impl_common {
    ($f:tt, $n:tt, $inner:ty; $($x:ident)*) => {
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
        }

        impl_inf_sup!(Vector<$f, $n>);
        impl_scalar_op_reverse!($f, $n, Mul, MulAssign, mul, mul_assign);
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
        assert_eq!(v.b(), 0.0);
        assert_eq!(v, [1.0, 0.0, 0.0].into());
        assert_eq!(<[f32; 3]>::from(v), [1.0, 0.0, 0.0]);
        assert_eq!(Vector3::from([1.0f32, 1.0, 1.0]), vec3(1.0f32, 1.0, 1.0));
    }

    #[test]
    fn index() {
        let v = vec3(0.0, 1.0, 2.0);
        assert_eq!(v[0], 0.0);
        assert_eq!(v[1], 1.0);
        assert_eq!(v[2], 2.0);
    }

    #[test]
    #[should_panic]
    fn index_out_of_bounds() {
        let v = vec3(0.0, 1.0, 2.0);
        v[3];
    }

    #[test]
    fn from_into() {
        let a = [1.0, 2.0, 3.0f32];
        let v: Vector3<f32> = a.into();
        assert_eq!(v, vec3(1.0, 2.0, 3.0));
        assert_eq!(<[f32; 3]>::from(v), a);
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

        assert_eq!(-1.0 * v, v * -1.0);
        assert_eq!(0.0 * v, v * 0.0);
        assert_eq!(1.0 * v, v * 1.0);
        assert_eq!(2.0 * v, v * 2.0);

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
