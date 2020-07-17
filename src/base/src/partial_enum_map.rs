use std::fmt::Debug;
use std::hash::Hash;
use std::iter::FromIterator;
use std::ops::{Index, IndexMut};

use derivative::Derivative;
use enum_map::{Enum, EnumMap};

/// A statically-sized partial mapping whose keys are members of an
/// enum.
// TODO: Impl into_iter, drain, etc. when [V; N] implements into_iter.
#[derive(Derivative)]
#[derivative(
    Clone(bound = "K::Array: Clone"),
    Copy(bound = "K::Array: Copy"),
    Default(bound = ""),
    Debug(bound = "K: Debug, V: Debug"),
    Eq(bound = "V: Eq"),
    Hash(bound = "V: Hash"),
    PartialEq(bound = "V: PartialEq"),
)]
pub struct PartialEnumMap<K: Enum<Option<V>>, V> {
    inner: EnumMap<K, Option<V>>,
}

impl<K: Enum<Option<V>>, V> PartialEnumMap<K, V> {
    #[inline(always)]
    pub fn new() -> Self {
        Default::default()
    }

    #[inline(always)]
    pub fn keys(&self) -> impl Iterator<Item = K> + '_ {
        self.iter().map(|(k, _)| k)
    }

    #[inline(always)]
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.inner.values().filter_map(Option::as_ref)
    }

    #[inline(always)]
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.inner.values_mut().filter_map(Option::as_mut)
    }

    #[inline(always)]
    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> {
        self.inner.iter().filter_map(|(k, v)| Some((k, v.as_ref()?)))
    }

    #[inline(always)]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> {
        self.inner.iter_mut().filter_map(|(k, v)| Some((k, v.as_mut()?)))
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        self.inner.clear()
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.inner.values().all(|x| x.is_none())
    }

    #[inline(always)]
    pub fn get(&self, key: K) -> Option<&V> {
        self.inner[key].as_ref()
    }

    #[inline(always)]
    pub fn get_mut(&mut self, key: K) -> Option<&mut V> {
        self.inner[key].as_mut()
    }

    #[inline(always)]
    pub fn contains_key(&self, key: K) -> bool {
        self.inner[key].is_some()
    }

    #[inline(always)]
    pub fn insert(&mut self, key: K, value: V) {
        self.inner[key] = Some(value);
    }

    #[inline(always)]
    pub fn remove(&mut self, key: K) -> Option<V> {
        self.inner[key].take()
    }
}

impl<K: Enum<Option<V>>, V> Index<K> for PartialEnumMap<K, V> {
    type Output = V;
    #[inline(always)]
    fn index(&self, key: K) -> &Self::Output {
        self.inner.index(key).as_ref().unwrap()
    }
}

impl<K: Enum<Option<V>>, V> IndexMut<K> for PartialEnumMap<K, V> {
    #[inline(always)]
    fn index_mut(&mut self, key: K) -> &mut Self::Output {
        self.inner.index_mut(key).as_mut().unwrap()
    }
}

impl<K: Enum<Option<V>>, V> Extend<(K, V)> for PartialEnumMap<K, V> {
    #[inline(always)]
    fn extend<I>(&mut self, iter: I)
        where I: IntoIterator<Item = (K, V)>
    {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

impl<K: Enum<Option<V>>, V> FromIterator<(K, V)> for PartialEnumMap<K, V> {
    #[inline(always)]
    fn from_iter<I>(iter: I) -> Self
        where I: IntoIterator<Item = (K, V)>
    {
        let mut this = Self::new();
        this.extend(iter);
        this
    }
}

#[macro_export]
macro_rules! partial_map {
    ($($key:expr => $val:expr),*$(,)?) => {
        {
            let mut map = $crate::PartialEnumMap::new();
            $(map.insert($key, $val);)*
            map
        }
    }
}

#[macro_export]
macro_rules! partial_map_opt {
    ($($key:expr => $val:expr),*$(,)?) => {
        {
            let mut map = $crate::PartialEnumMap::new();
            $(if let Some(val) = $val { map.insert($key, val); })*
            map
        }
    }
}

#[cfg(test)]
mod tests {
    use enum_map::Enum;
    use super::*;
    use self::Color::*;

    #[derive(Clone, Copy, Debug, Enum, Eq, PartialEq)]
    enum Color {
        Red,
        Green,
        Blue,
    }

    #[test]
    fn insert_remove() {
        let mut map: PartialEnumMap<Color, u32> = PartialEnumMap::new();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);

        map.insert(Red, 12);
        assert_eq!(map.get(Red), Some(&12));
        assert_eq!(map[Red], 12);
        assert!(!map.is_empty());
        assert_eq!(map.len(), 1);
        assert!(map.contains_key(Red));
        assert!(!map.contains_key(Green));

        map.insert(Red, 11);
        assert_eq!(map.get(Red), Some(&11));
        assert_eq!(map[Red], 11);
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(Green), None);
        assert_eq!(map.get(Blue), None);

        map.insert(Green, 2);
        assert_eq!(map.len(), 2);
        map.insert(Blue, 3);
        assert_eq!(map[Red], 11);
        assert_eq!(map[Green], 2);
        assert_eq!(map[Blue], 3);
        assert_eq!(map.len(), 3);
        assert!(map.contains_key(Green));
        assert!(map.contains_key(Blue));

        map.remove(Green);
        assert_eq!(map.get(Green), None);
        assert_eq!(map[Red], 11);
        assert_eq!(map.len(), 2);

        map.clear();
        assert!(map.is_empty());
    }

    #[test]
    fn iter() {
        let pairs = [(Red, 2u32), (Blue, 3)];
        let map: PartialEnumMap<_, _> = pairs.iter().copied().collect();
        assert_eq!(map[Red], 2);
        assert!(!map.contains_key(Green));
        assert_eq!(map[Blue], 3);
        assert_eq!(map, partial_map! {
            Red => 2,
            Blue => 3,
        });
        assert_eq!(map.keys().collect::<Vec<_>>(), vec![Red, Blue]);

        let mut map2 = PartialEnumMap::new();
        map2.insert(Green, 4);
        map2.extend(pairs.iter().copied());
        assert_eq!(map[Red], map2[Red]);
        assert_eq!(map2[Green], 4);
        assert_eq!(map[Blue], map2[Blue]);
        assert_eq!(map2.keys().collect::<Vec<_>>(), vec![Red, Green, Blue]);

        assert_eq!(map.values().copied().collect::<Vec<_>>(), vec![2, 3]);
        assert_eq!(
            map.iter().collect::<Vec<_>>(),
            vec![(Red, &2), (Blue, &3)],
        );
    }
}
