use std::collections::HashMap;
use std::hash::{BuildHasher, Hash};
use std::ops;

use derivative::*;
use prelude::num::*;

/// A vector indexed by a hashable type. Missing keys are treated as
/// zero, so the result is comparable to R^∞.
#[derive(Derivative)]
#[derivative(Clone(bound="HashMap<K, V, S>: Clone"))]
#[derivative(Debug(bound="HashMap<K, V, S>: std::fmt::Debug"))]
#[derivative(Default(bound="HashMap<K, V, S>: Default"))]
pub struct HashVector<K, V, S = fnv::FnvBuildHasher> {
    inner: HashMap<K, V, S>,
}

impl<K, V, S> From<HashMap<K, V, S>> for HashVector<K, V, S> {
    fn from(map: HashMap<K, V, S>) -> Self {
        HashVector { inner: map }
    }
}

impl<K, V, S> Into<HashMap<K, V, S>> for HashVector<K, V, S> {
    fn into(self) -> HashMap<K, V, S> {
        self.inner
    }
}

impl<K, V, S> Zero for HashVector<K, V, S> where Self: Default {
    fn zero() -> Self {
        Default::default()
    }
}

impl<K, V> HashVector<K, V> where Self: Default {
    pub fn new() -> Self {
        Default::default()
    }
}

impl<K, V, S> HashVector<K, V, S> {
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.inner.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut V)> {
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

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }
}

impl<K: Hash + Eq, V> HashVector<K, V> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity_and_hasher(capacity, Default::default())
    }
}

impl<K: Hash + Eq, V, S: BuildHasher> HashVector<K, V, S> {
    pub fn with_capacity_and_hasher(capacity: usize, hash_builder: S) -> Self {
        HashMap::with_capacity_and_hasher(capacity, hash_builder).into()
    }

    pub fn insert(&mut self, key: K, val: V) {
        self.inner.insert(key, val);
    }
}

impl<K: Hash + Eq, V: Zero, S: BuildHasher> HashVector<K, V, S> {
    /// Returns the value associated with a key, or zero if no value has
    /// been explicitly inserted.
    pub fn get<Q>(&self, key: &Q) -> V
    where
        Q: Hash + Eq,
        K: std::borrow::Borrow<Q>,
        V: Clone,
    {
        self.inner.get(key).cloned().unwrap_or_else(Zero::zero)
    }

    /// Returns a mutable reference to the value associated with a key.
    /// If no value has previously been inserted for this key, it will
    /// be set to zero.
    // TODO: I would like a plain `index` function, but there is no way
    // yet to get a 'static reference to the Default provided value.
    // Also, we cannot implement IndexMut without Index.
    // Note: We take the key by reference since in theory one may wish
    // to defer cloning until the hash entry is known to be vacant.
    pub fn index_mut<Q>(&mut self, key: &Q) -> &mut V
    where
        K: std::borrow::Borrow<Q>,
        Q: Hash + Eq + ToOwned<Owned = K>,
    {
        self.inner.entry(key.to_owned()).or_insert_with(Zero::zero)
    }

    /// Like index_mut, but takes the key by value.
    pub fn index_by_val_mut(&mut self, key: K) -> &mut V {
        self.inner.entry(key).or_insert_with(Zero::zero)
    }

    /// Removes keys which are redundantly set to zero, potentially
    /// freeing up memory.
    pub fn trim_zero(&mut self)
        where V: PartialEq
    {
        self.inner.retain(|_, v| v != &zero())
    }
}

impl<K, V, S> PartialEq for HashVector<K, V, S>
where
    K: Hash + Eq,
    V: PartialEq + Zero + Clone,
    S: BuildHasher,
{
    fn eq(&self, other: &Self) -> bool {
        for (k, v) in self.iter() {
            if v != &other.get(k) {
                return false;
            }
        }

        for (k, v) in other.iter() {
            if v != &self.get(k) {
                return false;
            }
        }

        true
    }
}

impl<K, V, S> Eq for HashVector<K, V, S>
where
    Self: PartialEq,
    V: Eq,
{}

macro_rules! impl_un_op {
    ($Op:ident, $op:ident) => {
        // Moving
        impl<K, V, S> ops::$Op for HashVector<K, V, S>
        where
            K: Hash + Eq,
            V: ops::$Op<Output = V> + Default,
            S: BuildHasher,
        {
            type Output = Self;
            fn $op(mut self) -> Self::Output {
                for v in self.values_mut() {
                    // Work around lack of in-place version of $Op
                    let u = std::mem::take(v);
                    *v = ops::$Op::$op(u);
                }
                self
            }
        }

        // Copying
        impl<'lhs, K, V, S> ops::$Op for &'lhs HashVector<K, V, S>
        where
            HashVector<K, V, S>: ops::$Op<Output = HashVector<K, V, S>> + Clone
        {
            type Output = HashVector<K, V, S>;
            fn $op(self) -> Self::Output {
                ops::$Op::$op(self.clone())
            }
        }
    }
}

macro_rules! impl_vector_op {
    ($Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident) => {
        impl<'rhs, K, V, S> ops::$OpAssign<&'rhs HashVector<K, V, S>> for
            HashVector<K, V, S>
        where
            K: Hash + Eq + Clone,
            V: ops::$OpAssign<&'rhs V> + Zero,
            S: BuildHasher,
        {
            fn $op_assign(&mut self, other: &'rhs HashVector<K, V, S>) {
                // Perf note: if other is much larger than self, it
                // would be faster to clone it and perform the reverse
                // assignment than to rehash every key as we do here.
                for (k, v) in other.iter() {
                    ops::$OpAssign::$op_assign(self.index_mut(k), v);
                }
            }
        }

        impl<K, V, S> ops::$OpAssign<HashVector<K, V, S>> for
            HashVector<K, V, S>
        where
            K: Hash + Eq + Clone,
            V: ops::$OpAssign + Zero,
            S: BuildHasher,
        {
            fn $op_assign(&mut self, other: HashVector<K, V, S>) {
                // Perf note above is relevant here.
                for (k, v) in other.into_iter() {
                    ops::$OpAssign::$op_assign(self.index_by_val_mut(k), v);
                }
            }
        }

        // Here as well.
        impl_bin_ops!(
            HashVector<K, V, S>,
            $Op, $OpAssign, $op, $op_assign,
        );
    }
}

macro_rules! impl_scalar_op {
    ($Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident) => {
        impl<K, V, S> ops::$OpAssign<V> for HashVector<K, V, S>
            where V: ops::$OpAssign + Clone,
        {
            fn $op_assign(&mut self, other: V) {
                for v in self.values_mut() {
                    ops::$OpAssign::$op_assign(v, other.clone());
                }
            }
        }

        // Moving
        impl<K, V, S> ops::$Op<V> for HashVector<K, V, S>
            where HashVector<K, V, S>: ops::$OpAssign<V>
        {
            type Output = HashVector<K, V, S>;
            fn $op(mut self, other: V) -> Self::Output {
                ops::$OpAssign::$op_assign(&mut self, other);
                self
            }
        }

        // Copying
        impl<'this, K, V, S> ops::$Op<V> for &'this HashVector<K, V, S>
            where HashVector<K, V, S>: ops::$OpAssign<V> + Clone
        {
            type Output = HashVector<K, V, S>;
            fn $op(self, other: V) -> Self::Output {
                let mut vec = self.clone();
                ops::$OpAssign::$op_assign(&mut vec, other);
                vec
            }
        }
    }
}

impl_un_op!(Neg, neg);
impl_vector_op!(Add, AddAssign, add, add_assign);
impl_vector_op!(Sub, SubAssign, sub, sub_assign);
impl_scalar_op!(Mul, MulAssign, mul, mul_assign);
impl_scalar_op!(Div, DivAssign, div, div_assign);
impl_scalar_op!(Rem, RemAssign, rem, rem_assign);

#[cfg(test)]
mod tests {
    use super::*;
    use self::Color::*;

    macro_rules! hash_vec {
        ($($key:expr => $val:expr),*$(,)*) => {
            {
                let mut map = $crate::HashVector::new();
                $(map.insert($key, $val);)*
                map
            }
        }
    }

    #[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
    enum Color {
        Red,
        Green,
        Blue,
    }

    #[test]
    fn i32_ops_test() {
        let mut x = hash_vec!(Red => 1i32, Green => 2, Blue => 3);
        assert_eq!(*x.index_mut(&Red), 1);
        *x.index_mut(&Red) += 1;
        assert_eq!(x.get(&Red), 2);
        assert_eq!(x.get(&Red), x.get(&Green));

        let mut y = hash_vec!(Red => 0i32);
        assert_eq!(y.get(&Green), 0);
        assert_eq!(y.len(), 1);
        assert_eq!(*y.index_mut(&Green), 0);
        assert_eq!(y.len(), 2);

        assert_eq!(hash_vec!(Red => 1i32, Blue => 0), hash_vec!(Red => 1i32));

        let mut a = hash_vec!(Red => -1i32, Green => 0, Blue => 1);
        let b = hash_vec!(Red => 1i32, Green => 0, Blue => -1);

        assert_eq!(a, a);
        assert_eq!(b, b);
        assert_eq!(a, -b.clone());
        assert_eq!(b, -&a);
        assert_eq!([a.get(&Red), a.get(&Green), a.get(&Blue)], [-1, 0, 1]);

        assert_eq!(&a + &b, HashVector::new());
        assert_eq!(&a - &b, hash_vec!(Red => -2i32, Blue => 2i32));
        assert_eq!(&a - &b, &a * 2);
        assert_eq!(&a + (&b * 2), b);

        assert_eq!(&a * 0, zero());
        assert_eq!(&a * 1, a);
        assert_eq!(&a * -1, b);

        let a_0 = a.clone();
        a *= 2;
        assert_eq!(&a % 2, zero());
        assert_eq!(&a / 2, a_0);
    }
}
