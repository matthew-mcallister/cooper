use std::fmt::{self, Debug};
use std::ops::*;

use derivative::Derivative;
use num::{One, Zero};
use packed_simd::{f32x2, f32x4, shuffle};

use crate::VectorOps;

pub trait Simd<const N: usize>: Sized {
    type Vector: Copy + Debug + Default + VectorOps<Self>;
}

type Inner<const N: usize> = <f32 as Simd<N>>::Vector;

/// A small, SIMD-backed mathematical vector of floats.
///
/// Note that objects of this type may be padded for optimal
/// alignment---for example, `Vector3` is 16 bytes in size and
/// alignment. However, it is possible to load a vector from an
/// unaligned primitive array.
#[derive(Derivative)]
#[derivative(
    Clone(bound = ""),
    Copy(bound = ""),
    Debug(bound = ""),
    Default(bound = ""),
)]
pub struct Vector<const N: usize>(Inner<N>)
    where f32: Simd<N>;

pub type Vector2 = Vector<2>;
pub type Vector3 = Vector<3>;
pub type Vector4 = Vector<4>;

/// This trait provides basic vector methods that are usable with
/// generics.
pub trait SimdOps<const N: usize>:
    AsRef<[f32; N]> + AsMut<[f32; N]> + Copy + Debug + Default + PartialEq
        + VectorOps<f32> + Index<usize, Output = f32> + IndexMut<usize>
{
    fn splat(scalar: f32) -> Self;
    fn load(src: &[f32; N]) -> Self;
    fn store(self, dst: &mut [f32; N]);
    fn sum(self) -> f32;
    fn product(self) -> f32;
    fn le(self, other: Self) -> bool;
    fn lt(self, other: Self) -> bool;
    fn ge(self, other: Self) -> bool;
    fn gt(self, other: Self) -> bool;
    fn inf(self, other: Self) -> Self;
    fn sup(self, other: Self) -> Self;

    #[inline(always)]
    fn dot(self, other: Self) -> f32 {
        (self * other).sum()
    }

    #[inline(always)]
    fn length_sq(self) -> f32 {
        self.dot(self)
    }

    #[inline(always)]
    fn length(self) -> f32 {
        self.length_sq().sqrt()
    }

    #[inline(always)]
    fn normalized(mut self) -> Self {
        self /= self.length();
        self
    }

    #[inline(always)]
    fn normalize(&mut self) -> f32 {
        let length = self.length();
        *self /= length;
        length
    }
}

pub trait SimdArray<const N: usize> = where
    f32: Simd<N>,
    Vector<N>: SimdOps<N>,
    ;

pub trait Swizzle2: Sized {
    fn x(self) -> f32;
    fn y(self) -> f32;

    #[inline(always)]
    fn r(self) -> f32 { self.x() }
    #[inline(always)]
    fn g(self) -> f32 { self.y() }
    #[inline(always)]
    fn s(self) -> f32 { self.x() }
    #[inline(always)]
    fn t(self) -> f32 { self.y() }
}

pub trait Swizzle3: Swizzle2 {
    fn xyz(self) -> Vector3;
    fn yzx(self) -> Vector3;
    fn zxy(self) -> Vector3;
    fn xyz_(self) -> Vector4;
    fn xyz0(self) -> Vector4;
    fn xyz1(self) -> Vector4;

    fn z(self) -> f32;
    #[inline(always)]
    fn b(self) -> f32 { self.z() }
    #[inline(always)]
    fn p(self) -> f32 { self.z() }
}

pub trait Swizzle4: Swizzle3 {
    fn w(self) -> f32;
    #[inline(always)]
    fn a(self) -> f32 { self.w() }
    #[inline(always)]
    fn q(self) -> f32 { self.w() }
}

impl<const N: usize> fmt::Display for Vector<N>
where
    f32: Simd<N>,
    Self: AsRef<[f32; N]>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let elems = self.as_ref();
        write!(f, "[{}", elems[0])?;
        for i in 1..N {
            write!(f, ", {}", elems[i])?;
        }
        write!(f, "]")
    }
}

const fn mask(n: usize) -> u8 {
    if n < 32 { !(!0u8 << n) } else { 0 }
}

macro_rules! impl_common {
    ($n:tt) => {
        #[inline(always)]
        fn le(self, other: Self) -> bool {
            const MASK: u8 = mask($n);
            (self.0.le(other.0).bitmask() & MASK) == MASK
        }

        #[inline(always)]
        fn lt(self, other: Self) -> bool {
            const MASK: u8 = mask($n);
            (self.0.lt(other.0).bitmask() & MASK) == MASK
        }

        #[inline(always)]
        fn ge(self, other: Self) -> bool {
            const MASK: u8 = mask($n);
            (self.0.ge(other.0).bitmask() & MASK) == MASK
        }

        #[inline(always)]
        fn gt(self, other: Self) -> bool {
            const MASK: u8 = mask($n);
            (self.0.gt(other.0).bitmask() & MASK) == MASK
        }

        #[inline(always)]
        fn inf(self, other: Self) -> Self {
            Self(self.0.min(other.0))
        }

        #[inline(always)]
        fn sup(self, other: Self) -> Self {
            Self(self.0.max(other.0))
        }
    }
}

macro_rules! impl_scalar_pot {
    ($scalar:tt, $n:tt, $vec:tt; $($x:ident)*) => {
        impl Simd<$n> for $scalar {
            type Vector = $vec;
        }

        impl PartialEq for Vector<$n> {
            #[inline(always)]
            fn eq(&self, other: &Self) -> bool {
                const MASK: u8 = mask($n);
                ($vec::eq(self.0, other.0).bitmask() & MASK) == MASK
            }

            #[inline(always)]
            fn ne(&self, other: &Self) -> bool {
                const MASK: u8 = mask($n);
                ($vec::ne(self.0, other.0).bitmask() & MASK) == MASK
            }
        }

        impl SimdOps<$n> for Vector<$n> {
            #[inline(always)]
            fn splat(scalar: $scalar) -> Self {
                Self($vec::splat(scalar))
            }

            #[inline(always)]
            fn load(src: &[$scalar; $n]) -> Self {
                Self($vec::from_slice_unaligned(&src[..]))
            }

            #[inline(always)]
            fn store(self, dst: &mut [$scalar; $n]) {
                self.0.write_to_slice_unaligned(&mut dst[..])
            }

            #[inline(always)]
            fn sum(self) -> $scalar {
                $vec::sum(self.0)
            }

            #[inline(always)]
            fn product(self) -> $scalar {
                $vec::product(self.0)
            }

            impl_common!($n);
        }
    }
}

impl_scalar_pot!(f32, 2, f32x2; x y);
impl_scalar_pot!(f32, 4, f32x4; x y z w);

impl Simd<3> for f32 {
    type Vector = f32x4;
}

impl PartialEq for Vector3 {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        const MASK: u8 = mask(3);
        (f32x4::eq(self.0, other.0).bitmask() & MASK) == MASK
    }

    #[inline(always)]
    fn ne(&self, other: &Self) -> bool {
        const MASK: u8 = mask(3);
        (f32x4::ne(self.0, other.0).bitmask() & MASK) == MASK
    }
}

impl SimdOps<3> for Vector3 {
    #[inline(always)]
    fn splat(scalar: f32) -> Self {
        Self(f32x4::splat(scalar))
    }

    #[inline(always)]
    fn load(&[x, y, z]: &[f32; 3]) -> Self {
        Self(f32x4::new(x, y, z, z))
    }

    #[inline(always)]
    fn store(self, array: &mut [f32; 3]) {
        *array = *self.as_ref();
    }

    #[inline(always)]
    fn sum(self) -> f32 {
        self.0.extract(0)
            + self.0.extract(1)
            + self.0.extract(2)
    }

    #[inline(always)]
    fn product(self) -> f32 {
        self.0.extract(0)
            * self.0.extract(1)
            * self.0.extract(2)
    }

    impl_common!(3);
}

#[inline(always)]
pub fn vec<const N: usize>(elems: [f32; N]) -> Vector<N>
where
    f32: Simd<N>,
    [f32; N]: Into<Vector<N>>,
{
    elems.into()
}

macro_rules! impl_vecn {
    ($vecn:ident, $N:expr, $($arg:ident),*) => {
        #[inline(always)]
        pub fn $vecn($($arg: f32,)*) -> Vector<$N>
            where [f32; $N]: Into<Vector<$N>>
        {
            vec([$($arg,)*])
        }
    }
}

impl_vecn!(vec2, 2, a, b);
impl_vecn!(vec3, 3, a, b, c);
impl_vecn!(vec4, 4, a, b, c, d);

impl<const N: usize> Vector<N>
    where f32: SimdArray<N>
{
    #[inline(always)]
    pub fn splat(scalar: f32) -> Self {
        SimdOps::splat(scalar)
    }

    #[inline(always)]
    pub fn load(src: &[f32; N]) -> Self {
        SimdOps::load(src)
    }

    #[inline(always)]
    pub fn store(self, dst: &mut [f32; N]) {
        SimdOps::store(self, dst)
    }

    #[inline(always)]
    pub fn sum(self) -> f32 {
        SimdOps::sum(self)
    }

    #[inline(always)]
    pub fn product(self) -> f32 {
        SimdOps::product(self)
    }

    #[inline(always)]
    pub fn le(self, other: Self) -> bool {
        SimdOps::le(self, other)
    }

    #[inline(always)]
    pub fn lt(self, other: Self) -> bool {
        SimdOps::lt(self, other)
    }

    #[inline(always)]
    pub fn ge(self, other: Self) -> bool {
        SimdOps::ge(self, other)
    }

    #[inline(always)]
    pub fn gt(self, other: Self) -> bool {
        SimdOps::gt(self, other)
    }

    /// Returns the infimum of two vectors, which is the minimum
    /// of the two taken component-wise.
    #[inline(always)]
    fn inf(self, other: Self) -> Self {
        SimdOps::inf(self, other)
    }

    /// Returns the supremum of two vectors, which is the
    /// maximum of the two taken component-wise.
    #[inline(always)]
    fn sup(self, other: Self) -> Self {
        SimdOps::sup(self, other)
    }

    #[inline(always)]
    pub fn dot(self, other: Self) -> f32 {
        SimdOps::dot(self, other)
    }

    #[inline(always)]
    pub fn length_sq(self) -> f32 {
        SimdOps::length_sq(self)
    }

    #[inline(always)]
    pub fn length(self) -> f32 {
        SimdOps::length(self)
    }

    #[inline(always)]
    pub fn normalized(self) -> Self {
        SimdOps::normalized(self)
    }

    #[inline(always)]
    pub fn normalize(&mut self) -> f32 {
        SimdOps::normalize(self)
    }
}

impl<const N: usize> crate::InfSup for Vector<N>
    where f32: SimdArray<N>
{
    impl_inf_sup!();
}

impl<const N: usize> Zero for Vector<N>
    where f32: SimdArray<N>
{
    #[inline(always)]
    fn zero() -> Self {
        Self::splat(0.0)
    }
}

impl<const N: usize> One for Vector<N>
    where f32: SimdArray<N>
{
    #[inline(always)]
    fn one() -> Self {
        Self::splat(1.0)
    }
}

macro_rules! impl_un_op {
    ($Op:ident, $op:ident) => {
        impl<const N: usize> $Op for Vector<N>
            where f32: Simd<N>
        {
            type Output = Vector<N>;
            #[inline(always)]
            fn $op(self) -> Vector<N> {
                Vector($Op::$op(self.0))
            }
        }
    }
}

impl_un_op!(Neg, neg);

// TODO?: Impls taking references
macro_rules! impl_vec_op {
    ($Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident) => {
        impl<const N: usize> $Op<Vector<N>> for Vector<N>
            where f32: Simd<N>
        {
            type Output = Vector<N>;
            #[inline(always)]
            fn $op(self, other: Vector<N>) -> Vector<N> {
                Vector(<Inner<N> as $Op>::$op(self.0, other.0))
            }
        }

        impl<const N: usize> $OpAssign<Vector<N>> for Vector<N>
            where f32: Simd<N>
        {
            #[inline(always)]
            fn $op_assign(&mut self, other: Vector<N>) {
                <Inner<N> as $OpAssign>::$op_assign(
                    &mut self.0, other.0);
            }
        }
    }
}

impl_vec_op!(Add, AddAssign, add, add_assign);
impl_vec_op!(Sub, SubAssign, sub, sub_assign);
impl_vec_op!(Mul, MulAssign, mul, mul_assign);
impl_vec_op!(Div, DivAssign, div, div_assign);

impl<const N: usize> std::iter::Sum for Vector<N>
    where f32: SimdArray<N>
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
        impl<const N: usize> $Op<f32> for Vector<N>
            where f32: Simd<N>
        {
            type Output = Vector<N>;
            #[inline(always)]
            fn $op(self, scalar: f32) -> Vector<N> {
                Self($Op::$op(self.0, scalar))
            }
        }

        impl<const N: usize> $OpAssign<f32> for Vector<N>
            where f32: Simd<N>
        {
            #[inline(always)]
            fn $op_assign(&mut self, scalar: f32) {
                $OpAssign::$op_assign(&mut self.0, scalar)
            }
        }
    }
}

impl_scalar_op!(Mul, MulAssign, mul, mul_assign);
impl_scalar_op!(Div, DivAssign, div, div_assign);

impl<const N: usize> Mul<Vector<N>> for f32
    where f32: Simd<N>
{
    type Output = Vector<N>;
    #[inline(always)]
    fn mul(self, vector: Vector<N>) -> Vector<N> {
        Mul::mul(vector, self)
    }
}

macro_rules! impl_array_ops {
    ($n:tt) => {
        impl From<[f32; $n]> for Vector<$n>
            where f32: Simd<$n>
        {
            #[inline(always)]
            fn from(array: [f32; $n]) -> Self {
                Self::load(&array)
            }
        }

        impl From<Vector<$n>> for [f32; $n] {
            #[inline(always)]
            fn from(vec: Vector<$n>) -> Self {
                let mut array: Self = [0.0; $n];
                vec.store(&mut array);
                array
            }
        }

        impl AsRef<[f32; $n]> for Vector<$n> {
            #[inline(always)]
            fn as_ref(&self) -> &[f32; $n] {
                unsafe { &*(self as *const Self as *const [f32; $n]) }
            }
        }

        impl AsMut<[f32; $n]> for Vector<$n>
            where f32: Simd<$n>
        {
            #[inline(always)]
            fn as_mut(&mut self) -> &mut [f32; $n] {
                unsafe { &mut *(self as *mut Self as *mut [f32; $n]) }
            }
        }

        impl<I> Index<I> for Vector<$n>
        where
            f32: Simd<$n>,
            [f32]: Index<I>,
        {
            type Output = <[f32] as Index<I>>::Output;

            #[inline(always)]
            fn index(&self, idx: I) -> &Self::Output {
                self.as_ref().index(idx)
            }
        }

        impl<I> IndexMut<I> for Vector<$n>
        where
            f32: Simd<$n>,
            [f32]: IndexMut<I>,
        {
            #[inline(always)]
            fn index_mut(&mut self, idx: I) -> &mut Self::Output {
                self.as_mut().index_mut(idx)
            }
        }
    }
}

impl_array_ops!(2);
impl_array_ops!(3);
impl_array_ops!(4);

macro_rules! impl_accessor {
    ($x:ident $i:expr) => {
        #[inline(always)]
        fn $x(self) -> f32 {
            self.0.extract($i)
        }
    }
}

impl Swizzle2 for Vector2 {
    impl_accessor!(x 0);
    impl_accessor!(y 1);
}

impl Swizzle2 for Vector3 {
    impl_accessor!(x 0);
    impl_accessor!(y 1);
}

impl Swizzle3 for Vector3 {
    impl_accessor!(z 2);

    #[inline(always)]
    fn xyz(self) -> Vector3 {
        self
    }

    #[inline(always)]
    fn yzx(self) -> Vector3 {
        Vector(shuffle!(self.0, self.0, [1, 2, 0, 3]))
    }

    #[inline(always)]
    fn zxy(self) -> Vector3 {
        Vector(shuffle!(self.0, self.0, [2, 0, 1, 3]))
    }

    #[inline(always)]
    fn xyz_(self) -> Vector4 {
        Vector(self.0)
    }

    #[inline(always)]
    fn xyz0(self) -> Vector4 {
        Vector(self.0.replace(3, 0.0))
    }

    #[inline(always)]
    fn xyz1(self) -> Vector4 {
        Vector(self.0.replace(3, 1.0))
    }
}

impl Swizzle2 for Vector4 {
    impl_accessor!(x 0);
    impl_accessor!(y 1);
}

impl Swizzle3 for Vector4 {
    impl_accessor!(z 2);

    #[inline(always)]
    fn xyz(self) -> Vector3 {
        Vector(self.0)
    }

    #[inline(always)]
    fn yzx(self) -> Vector3 {
        self.xyz().yzx()
    }

    #[inline(always)]
    fn zxy(self) -> Vector3 {
        self.xyz().zxy()
    }

    #[inline(always)]
    fn xyz_(self) -> Vector4 {
        self
    }

    #[inline(always)]
    fn xyz0(self) -> Vector4 {
        Vector(self.0.replace(3, 0.0))
    }

    #[inline(always)]
    fn xyz1(self) -> Vector4 {
        Vector(self.0.replace(3, 1.0))
    }
}

impl Swizzle4 for Vector4 {
    impl_accessor!(w 3);
}

macro_rules! delegate_swizzles {
    ($trait:ident { $($swizzle:ident -> $ty:ty;)* }) => {
        impl<const N: usize> Vector<N>
        where
            f32: Simd<N>,
            Self: $trait,
        {
            $(
                #[inline(always)]
                pub fn $swizzle(self) -> $ty {
                    $trait::$swizzle(self)
                }
            )*
        }
    }
}

delegate_swizzles!(Swizzle2 {
    x -> f32;
    y -> f32;
    r -> f32;
    g -> f32;
    s -> f32;
    t -> f32;
});

delegate_swizzles!(Swizzle3 {
    xyz -> Vector3;
    yzx -> Vector3;
    zxy -> Vector3;
    xyz_ -> Vector4;
    xyz0 -> Vector4;
    xyz1 -> Vector4;
    z -> f32;
    b -> f32;
    p -> f32;
});

delegate_swizzles!(Swizzle4 {
    w -> f32;
    a -> f32;
    q -> f32;
});

macro_rules! impl_po2 {
    ($n:tt, $inner:tt; $($x:ident)*) => {
        impl Vector<$n> {
            #[inline(always)]
            pub fn new($($x: f32),*) -> Self {
                Self($inner::new($($x),*))
            }
        }
    }
}

impl_po2!(2, f32x2; x y);
impl_po2!(4, f32x4; x y z w);

impl Vector3 {
    #[inline(always)]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        // N.B. This probably doesn't generate the best code every time.
        Self(f32x4::new(x, y, z, z))
    }

    #[inline(always)]
    pub fn cross(self, other: Self) -> Self {
        (self * other.yzx() - self.yzx() * other).yzx()
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
        let v: Vector3 = a.into();
        assert_eq!(v, vec3(1.0, 2.0, 3.0));
        assert_eq!(<[f32; 3]>::from(v), a);
    }

    #[test]
    fn vec_ops() {
        let v: Vector3 = [1.0, 0.0, 0.0].into();
        let u: Vector3 = vec([0.0, 1.0, 0.0]);
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
        assert_eq!(vec2(1.0, 2.0).sum(), 3.0);
        assert_eq!(vec2(1.0, 2.0).product(), 2.0);
        assert_eq!(vec3(1.0, 2.0, 3.0).sum(), 6.0);
        assert_eq!(vec3(1.0, 2.0, 3.0).product(), 6.0);

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
    fn sum() {
        let vecs: &[Vector2] = &[];
        assert_eq!(vecs.iter().copied().sum::<Vector2>(), Zero::zero());
        let vecs = &[
            vec2(0.0, 1.0),
            vec2(1.0, 2.0),
            vec2(2.0, 3.0),
            vec2(3.0, 4.0),
        ];
        assert_eq!(vecs.iter().copied().sum::<Vector2>(), vec2(6.0, 10.0));
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

        assert_eq!([].iter().cloned().inf_sup::<Vector2>(),
            InfSupResult::Empty);
        let vecs = &[vec2(1.0, 1.0)];
        assert_eq!(vecs.iter().cloned().inf_sup(),
            InfSupResult::Singleton(vec2(1.0, 1.0)));
    }
}
