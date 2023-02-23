#![allow(clippy::len_without_is_empty, clippy::should_implement_trait)]

use derivative::*;
use derive_more::*;
use enum_map::{Enum, EnumMap};

use crate::impl_bin_ops;
use crate::num::*;

/// Represents a numeric vector indexed by an enum.
#[derive(Derivative, From, Index, IndexMut)]
#[derivative(Clone(bound = "EnumMap<K, V>: Clone"))]
#[derivative(Copy(bound = "EnumMap<K, V>: Copy"))]
#[derivative(Debug(bound = "EnumMap<K, V>: std::fmt::Debug"))]
#[derivative(Default(bound = "EnumMap<K, V>: Default"))]
#[derivative(Eq(bound = "EnumMap<K, V>: Eq"))]
#[derivative(PartialEq(bound = "EnumMap<K, V>: PartialEq"))]
pub struct EnumVector<K: Enum<V>, V> {
    inner: EnumMap<K, V>,
}

impl<K: Enum<V>, V> Into<EnumMap<K, V>> for EnumVector<K, V> {
    fn into(self) -> EnumMap<K, V> {
        self.inner
    }
}

impl<K: Enum<V>, V: Copy> From<V> for EnumVector<K, V> {
    fn from(val: V) -> Self {
        Self::lift(val)
    }
}

impl<K: Enum<V>, V: Zero + Copy> Zero for EnumVector<K, V> {
    fn zero() -> Self {
        V::zero().into()
    }
}

impl<K: Enum<V>, V: One + Copy> One for EnumVector<K, V> {
    fn one() -> Self {
        V::one().into()
    }
}

impl<K: Enum<V>, V: Default> EnumVector<K, V> {
    pub fn new() -> Self {
        Default::default()
    }
}

impl<K: Enum<V>, V: Copy> EnumVector<K, V> {
    pub fn lift(elem: V) -> Self {
        Self::from_fn(|_| elem)
    }
}

impl<K: Enum<V>, V> EnumVector<K, V> {
    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> {
        self.inner.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> {
        self.inner.iter_mut()
    }

    pub fn into_iter(self) -> impl Iterator<Item = (K, V)> {
        self.inner.into_iter()
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.inner.values()
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.inner.values_mut()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn as_slice(&self) -> &[V] {
        self.inner.as_slice()
    }

    pub fn as_mut_slice(&mut self) -> &mut [V] {
        self.inner.as_mut_slice()
    }

    pub fn from_fn(f: impl FnMut(K) -> V) -> Self {
        Self { inner: f.into() }
    }
}

macro_rules! impl_un_op {
    ($Op:ident, $op:ident) => {
        impl<K: Enum<V>, V> std::ops::$Op for EnumVector<K, V>
        where
            V: std::ops::$Op<Output = V> + Copy,
        {
            type Output = Self;
            fn $op(mut self) -> Self::Output {
                for v in self.values_mut() {
                    *v = <V as std::ops::$Op>::$op(*v);
                }
                self
            }
        }
    };
}

macro_rules! impl_bin_op {
    ($Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident) => {
        // TODO: The V: Copy bound should not be necessary
        impl<K: Enum<V>, V> std::ops::$OpAssign for EnumVector<K, V>
            where V: std::ops::$OpAssign + Copy,
        {
            fn $op_assign(&mut self, other: Self) {
                for (k, v) in self.iter_mut() {
                    <V as std::ops::$OpAssign>::$op_assign(v, other[k]);
                }
            }
        }

        impl<'rhs, K: Enum<V>, V> std::ops::$OpAssign<&'rhs Self>
            for EnumVector<K, V>
            where V: std::ops::$OpAssign<&'rhs V>,
        {
            fn $op_assign(&mut self, other: &'rhs Self) {
                for (k, v) in self.iter_mut() {
                    <V as std::ops::$OpAssign<&'rhs V>>::$op_assign(v, &other[k]);
                }
            }
        }

        impl_bin_ops!(
            {K: Enum<V>, V}, {V: Copy},
            (EnumVector<K, V>), (EnumVector<K, V>),
            Copy,
            (std::ops::$Op), (std::ops::$OpAssign), $op, $op_assign,
        );

        impl<K: Enum<V>, V> std::ops::$OpAssign<V> for EnumVector<K, V>
            where V: std::ops::$OpAssign + Copy,
        {
            fn $op_assign(&mut self, other: V) {
                for v in self.values_mut() {
                    <V as std::ops::$OpAssign>::$op_assign(v, other);
                }
            }
        }

        impl<'rhs, K: Enum<V>, V> std::ops::$OpAssign<&'rhs V>
            for EnumVector<K, V>
            where V: std::ops::$OpAssign<&'rhs V>,
        {
            fn $op_assign(&mut self, other: &'rhs V) {
                for v in self.values_mut() {
                    <V as std::ops::$OpAssign<&'rhs V>>::$op_assign(v, other);
                }
            }
        }

        impl_bin_ops!(
            {K: Enum<V>, V}, {V: Copy},
            (EnumVector<K, V>), (V),
            Copy,
            (std::ops::$Op), (std::ops::$OpAssign), $op, $op_assign,
        );
    }
}

impl_un_op!(Neg, neg);
impl_un_op!(Not, not);
impl_bin_op!(Add, AddAssign, add, add_assign);
impl_bin_op!(Sub, SubAssign, sub, sub_assign);
impl_bin_op!(Mul, MulAssign, mul, mul_assign);
impl_bin_op!(Div, DivAssign, div, div_assign);
impl_bin_op!(Rem, RemAssign, rem, rem_assign);
impl_bin_op!(BitAnd, BitAndAssign, bitand, bitand_assign);
impl_bin_op!(BitOr, BitOrAssign, bitor, bitor_assign);
impl_bin_op!(BitXor, BitXorAssign, bitxor, bitxor_assign);

impl<K: Enum<V>, V> std::iter::Sum<(K, V)> for EnumVector<K, V>
where
    Self: Default,
    V: std::ops::AddAssign + Copy,
{
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = (K, V)>,
    {
        let mut vec = Self::default();
        for (k, v) in iter {
            vec[k] += v;
        }
        vec
    }
}

impl<K: Enum<V>, V> std::iter::FromIterator<(K, V)> for EnumVector<K, V>
where
    Self: Default,
{
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
    {
        let mut vec = Self::default();
        for (k, v) in iter {
            vec[k] = v;
        }
        vec
    }
}

#[macro_export]
macro_rules! enum_vec {
    ($($body:tt)*) => {
        $crate::EnumVector::from(enum_map!($($body)*))
    }
}

#[cfg(test)]
mod tests {
    use self::Color::*;
    use super::*;
    use enum_map::{enum_map, Enum};

    #[derive(Clone, Copy, Debug, Enum, Eq, PartialEq)]
    enum Color {
        Red,
        Green,
        Blue,
    }

    #[test]
    fn i32_ops_test() {
        let a = enum_vec!(Red => -1i32, Green => 0, Blue => 1);
        let b = enum_vec!(Red => 1i32, Green => 0, Blue => -1);

        assert_eq!(a, a);
        assert_eq!(b, b);
        assert_eq!(a, -b);
        assert_eq!(b, -a);
        assert_eq!(a + b, EnumVector::lift(0));
        assert_eq!([a[Red], a[Green], a[Blue]], [-1, 0, 1]);

        assert_eq!(a - b, a * 2);
        assert_eq!(a + b * 2, b);

        assert_eq!(
            EnumVector::lift(1) - a,
            enum_vec!(Red => 2i32, Green => 1, Blue => 0),
        );

        assert_eq!(a * 0, EnumVector::zero());
        assert_eq!(a * EnumVector::zero(), EnumVector::zero());
        assert_eq!(a * 1, a);
        assert_eq!(a * EnumVector::one(), a);

        let c = a * 2;
        assert_eq!(c % 2, zero());
        assert_eq!(c / 2, a);
    }
}
