use std::fmt::Debug;
use std::ops::*;

use base::num::{One, Zero};

/// A vector of `u32`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Uvector<const N: usize>([u32; N]);

pub type Uvector2 = Uvector<2>;
pub type Uvector3 = Uvector<3>;
pub type Uvector4 = Uvector<4>;

/// A vector of `i32`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ivector<const N: usize>([i32; N]);

pub type Ivector2 = Ivector<2>;
pub type Ivector3 = Ivector<3>;
pub type Ivector4 = Ivector<4>;

macro_rules! impl_common {
    ($vector:ident, $scalar:ident) => {
        impl<const N: usize> $vector<N> {
            #[inline(always)]
            pub fn splat(scalar: $scalar) -> Self {
                Self([scalar; N])
            }

            #[inline(always)]
            pub fn load(src: &[$scalar; N]) -> Self {
                Self(*src)
            }

            #[inline(always)]
            pub fn store(self, dst: &mut [$scalar; N]) {
                *dst = self.0;
            }

            #[inline(always)]
            pub fn sum(self) -> $scalar {
                self.0.iter().copied().sum()
            }

            #[inline(always)]
            pub fn product(self) -> $scalar {
                self.0.iter().copied().product()
            }

            #[inline(always)]
            pub fn le(self, other: Self) -> bool {
                self.0.iter().zip(other.0.iter()).all(|(&a, &b)| a <= b)
            }

            #[inline(always)]
            pub fn lt(self, other: Self) -> bool {
                self.0.iter().zip(other.0.iter()).all(|(&a, &b)| a < b)
            }

            #[inline(always)]
            pub fn ge(self, other: Self) -> bool {
                self.0.iter().zip(other.0.iter()).all(|(&a, &b)| a >= b)
            }

            #[inline(always)]
            pub fn gt(self, other: Self) -> bool {
                self.0.iter().zip(other.0.iter()).all(|(&a, &b)| a > b)
            }

            #[inline(always)]
            pub fn inf(mut self, other: Self) -> Self {
                for i in 0..N {
                    self[i] = self[i].min(other[i]);
                }
                self
            }

            #[inline(always)]
            pub fn sup(mut self, other: Self) -> Self {
                for i in 0..N {
                    self[i] = self[i].max(other[i]);
                }
                self
            }

            #[inline(always)]
            pub fn iter(&self) -> impl Iterator<Item = &$scalar> {
                self.as_ref().iter()
            }

            #[inline(always)]
            pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut $scalar> {
                self.as_mut().iter_mut()
            }

            #[inline(always)]
            pub fn map(mut self, mut f: impl FnMut($scalar) -> $scalar) -> Self {
                for i in 0..N {
                    self[i] = f(self[i]);
                }
                self
            }
        }

        impl<const N: usize> Default for $vector<N> {
            fn default() -> Self {
                Zero::zero()
            }
        }

        impl<const N: usize> Zero for $vector<N> {
            #[inline(always)]
            fn zero() -> Self {
                Self::splat(0)
            }
        }

        impl<const N: usize> One for $vector<N> {
            #[inline(always)]
            fn one() -> Self {
                Self::splat(1)
            }
        }

        impl_vec_op!($vector, Add, AddAssign, add, add_assign);
        impl_vec_op!($vector, Sub, SubAssign, sub, sub_assign);
        impl_vec_op!($vector, Mul, MulAssign, mul, mul_assign);
        impl_vec_op!($vector, Div, DivAssign, div, div_assign);
        impl_scalar_op!($vector, $scalar, Mul, MulAssign, mul, mul_assign);
        impl_scalar_op!($vector, $scalar, Div, DivAssign, div, div_assign);

        impl<const N: usize> Mul<$vector<N>> for $scalar {
            type Output = $vector<N>;
            #[inline(always)]
            fn mul(self, vector: $vector<N>) -> $vector<N> {
                Mul::mul(vector, self)
            }
        }

        impl<const N: usize> std::iter::Sum for $vector<N> {
            #[inline(always)]
            fn sum<I>(iter: I) -> Self
            where
                I: Iterator<Item = Self>,
            {
                iter.fold(Default::default(), Add::add)
            }
        }

        impl<const N: usize> From<[$scalar; N]> for $vector<N> {
            #[inline(always)]
            fn from(array: [$scalar; N]) -> Self {
                Self(array)
            }
        }

        impl<const N: usize> From<$vector<N>> for [$scalar; N] {
            #[inline(always)]
            fn from(vec: $vector<N>) -> Self {
                vec.0
            }
        }

        impl<const N: usize> AsRef<[$scalar; N]> for $vector<N> {
            #[inline(always)]
            fn as_ref(&self) -> &[$scalar; N] {
                &self.0
            }
        }

        impl<const N: usize> AsMut<[$scalar; N]> for $vector<N> {
            #[inline(always)]
            fn as_mut(&mut self) -> &mut [$scalar; N] {
                &mut self.0
            }
        }

        impl<I, const N: usize> Index<I> for $vector<N>
        where
            [$scalar]: Index<I>,
        {
            type Output = <[$scalar] as Index<I>>::Output;
            #[inline(always)]
            fn index(&self, idx: I) -> &Self::Output {
                self.as_ref().index(idx)
            }
        }

        impl<I, const N: usize> IndexMut<I> for $vector<N>
        where
            [$scalar]: IndexMut<I>,
        {
            #[inline(always)]
            fn index_mut(&mut self, idx: I) -> &mut Self::Output {
                self.as_mut().index_mut(idx)
            }
        }
    };
}

macro_rules! impl_un_op {
    ($vector:ident, $Op:ident, $op:ident) => {
        impl<const N: usize> $Op for $vector<N> {
            type Output = $vector<N>;
            #[inline(always)]
            fn $op(mut self) -> $vector<N> {
                for i in 0..N {
                    self[i] = $Op::$op(self[i]);
                }
                self
            }
        }
    };
}

macro_rules! impl_vec_op {
    ($vector:ident, $Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident) => {
        impl<const N: usize> $Op<$vector<N>> for $vector<N> {
            type Output = $vector<N>;
            #[inline(always)]
            fn $op(mut self, other: $vector<N>) -> $vector<N> {
                $OpAssign::$op_assign(&mut self, other);
                self
            }
        }

        impl<const N: usize> $OpAssign<$vector<N>> for $vector<N> {
            #[inline(always)]
            fn $op_assign(&mut self, other: $vector<N>) {
                for i in 0..N {
                    self[i] = $Op::$op(self[i], other[i]);
                }
            }
        }
    };
}

macro_rules! impl_scalar_op {
    (
        $vector:ident, $scalar:ident, $Op:ident, $OpAssign:ident, $op:ident,
        $op_assign:ident
    ) => {
        impl<const N: usize> $Op<$scalar> for $vector<N> {
            type Output = $vector<N>;
            #[inline(always)]
            fn $op(mut self, scalar: $scalar) -> $vector<N> {
                $OpAssign::$op_assign(&mut self, scalar);
                self
            }
        }

        impl<const N: usize> $OpAssign<$scalar> for $vector<N> {
            #[inline(always)]
            fn $op_assign(&mut self, scalar: $scalar) {
                for i in 0..N {
                    self[i] = $Op::$op(self[i], scalar);
                }
            }
        }
    };
}

impl_common!(Uvector, u32);
impl_common!(Ivector, i32);

impl_un_op!(Ivector, Neg, neg);

#[inline(always)]
pub fn uvec<const N: usize>(elems: [u32; N]) -> Uvector<N> {
    elems.into()
}

#[inline(always)]
pub fn ivec<const N: usize>(elems: [i32; N]) -> Ivector<N> {
    elems.into()
}

macro_rules! impl_vecn {
    (
        $vector:ident, $scalar:ident, $vec:ident, $vecn:ident, $N:expr,
        $($arg:ident),*
    ) => {
        #[inline(always)]
        pub fn $vecn($($arg: $scalar,)*) -> $vector<$N> {
            $vector::<$N>::new($($arg,)*)
        }

        impl $vector<$N> {
            #[inline(always)]
            pub fn new($($arg: $scalar,)*) -> Self {
                Self([$($arg,)*])
            }
        }
    }
}

impl_vecn!(Uvector, u32, uvec, uvec2, 2, x, y);
impl_vecn!(Uvector, u32, uvec, uvec3, 3, x, y, z);
impl_vecn!(Uvector, u32, uvec, uvec4, 4, x, y, z, w);
impl_vecn!(Ivector, i32, ivec, ivec2, 2, x, y);
impl_vecn!(Ivector, i32, ivec, ivec3, 3, x, y, z);
impl_vecn!(Ivector, i32, ivec, ivec4, 4, x, y, z, w);

macro_rules! impl_accessors {
    ($vector:ident<$scalar:ty> { $(($x:ident $i:expr))* }) => {
        impl $vector {
            $(
                #[inline(always)]
                pub fn $x(self) -> $scalar {
                    self[$i]
                }
            )*
        }
    }
}

impl_accessors!(Uvector2<u32> { (x 0) (y 1) });
impl_accessors!(Uvector3<u32> { (x 0) (y 1) (z 2) });
impl_accessors!(Uvector4<u32> { (x 0) (y 1) (z 2) (w 3) });
impl_accessors!(Ivector2<i32> { (x 0) (y 1) });
impl_accessors!(Ivector3<i32> { (x 0) (y 1) (z 2) });
impl_accessors!(Ivector4<i32> { (x 0) (y 1) (z 2) (w 3) });

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessors() {
        let v = Uvector3::new(1, 0, 0);
        assert_eq!(v.x(), 1);
        assert_eq!(v.z(), 0);
        assert_eq!(v, [1, 0, 0].into());
        assert_eq!(<[u32; 3]>::from(v), [1, 0, 0]);
        assert_eq!(Uvector3::from([1, 1, 1]), uvec3(1, 1, 1));
    }

    #[test]
    fn index() {
        let v = ivec3(0, 1, 2);
        assert_eq!(v[0], 0);
        assert_eq!(v[1], 1);
        assert_eq!(v[2], 2);
    }

    #[test]
    #[should_panic]
    fn index_out_of_bounds() {
        let v = uvec3(0, 1, 2);
        v[3];
    }

    #[test]
    fn from_into() {
        let a = [1, 2, 3];
        let v: Ivector3 = a.into();
        assert_eq!(v, ivec3(1, 2, 3));
        assert_eq!(<[i32; 3]>::from(v), a);
    }

    #[test]
    fn vec_ops() {
        let v: Ivector3 = [1, 0, 0].into();
        let u: Ivector3 = ivec([0, 1, 0]);
        assert_eq!(-v, ivec3(-1, 0, 0));
        assert_eq!(u + v, ivec3(1, 1, 0));
        assert_eq!(u - v, ivec3(-1, 1, 0));
        assert_eq!(u * v, Zero::zero());
        assert_eq!(v - v, Zero::zero());
        assert_eq!(v + v, ivec3(2, 0, 0));
        assert_eq!(ivec2(2, 1) / ivec2(1, 2), ivec2(2, 0));
    }

    #[test]
    fn scalar_ops() {
        let v = Ivector3::new(1, 0, 0);
        assert_eq!(v * 1, v);
        assert_eq!(v * 0, Zero::zero());
        assert_eq!(v * 2, v + v);
        assert_eq!(v * -1, -v);

        assert_eq!(-1 * v, v * -1);
        assert_eq!(0 * v, v * 0);
        assert_eq!(1 * v, v * 1);
        assert_eq!(2 * v, v * 2);

        let v = ivec3(2, 0, 0);
        assert_eq!(v / 1, v);
        assert_eq!(v / 2, v - v / 2);
        assert_eq!(v / -1, -v);
    }

    #[test]
    fn methods() {
        assert_eq!(uvec2(1, 2).sum(), 3);
        assert_eq!(uvec2(1, 2).product(), 2);
        assert_eq!(uvec3(1, 2, 3).sum(), 6);
        assert_eq!(uvec3(1, 2, 3).product(), 6);
        assert_eq!(ivec2(1, 2).map(Neg::neg), ivec2(-1, -2));
    }

    #[test]
    fn order() {
        assert!(uvec2(0, 0).le(Zero::zero()));
        assert!(uvec2(0, 0).ge(Zero::zero()));
        assert!(!uvec2(0, 0).lt(Zero::zero()));
        assert!(!uvec2(0, 0).gt(Zero::zero()));

        assert!(ivec2(0, 1).lt(ivec2(1, 2)));
        assert!(ivec2(0, 1).le(ivec2(1, 2)));
        assert!(!ivec2(0, 1).gt(ivec2(1, 2)));
        assert!(!ivec2(0, 1).ge(ivec2(1, 2)));
    }

    #[test]
    fn sum() {
        let vecs: &[Uvector2] = &[];
        assert_eq!(vecs.iter().copied().sum::<Uvector2>(), Zero::zero());
        let vecs = &[uvec2(0, 1), uvec2(1, 2), uvec2(2, 3), uvec2(3, 4)];
        assert_eq!(vecs.iter().copied().sum::<Uvector2>(), uvec2(6, 10));
    }
}
