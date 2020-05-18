use std::fmt;
use std::marker::PhantomData;

use derivative::Derivative;

use crate::pool::*;

#[derive(Derivative)]
#[derivative(
    Clone(bound=""), Copy(bound=""), Eq(bound=""), Hash(bound=""),
    PartialEq(bound=""),
)]
pub struct Id<T> {
    inner: PoolId,
    _ph: PhantomData<Box<T>>,
}

impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Id")
            .field("idx", &self.inner.idx)
            .field("gen", &self.inner.gen)
            .finish()
    }
}

impl<T> Id<T> {
    fn new(inner: PoolId) -> Self {
        Id { inner, _ph: PhantomData }
    }
}

#[derive(Debug)]
pub struct Node<T> {
    node_value: T,
}

impl<T> std::ops::Deref for Node<T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.node_value
    }
}

impl<T> std::ops::DerefMut for Node<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.node_value
    }
}

impl<T> Node<T> {
    fn new(value: T) -> Self {
        Node {
            node_value: value,
        }
    }
}

#[derive(Debug)]
pub struct NodeArray<T> {
    pool: Pool<Node<T>>,
}

impl<T> Default for NodeArray<T> {
    fn default() -> Self {
        NodeArray {
            pool: Default::default(),
        }
    }
}

impl<T> NodeArray<T> {
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }

    // Delegating methods

    #[inline]
    pub fn len(&self) -> u32 {
        self.pool.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn add(&mut self, value: T) -> Id<T> {
        Id::new(self.pool.add(Node::new(value)))
    }

    #[inline]
    pub fn remove(&mut self, id: Id<T>) -> Option<Node<T>> {
        self.pool.remove(id.inner)
    }

    #[inline]
    pub fn contains(&self, id: Id<T>) -> bool {
        self.pool.contains(id.inner)
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (Id<T>, &Node<T>)> {
        self.pool.iter().map(|(id, node)| (Id::new(id), node))
    }

    #[inline]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Id<T>, &mut Node<T>)> {
        self.pool.iter_mut().map(|(id, node)| (Id::new(id), node))
    }

    #[inline]
    pub fn get(&self, id: Id<T>) -> Option<&Node<T>> {
        self.pool.get(id.inner)
    }

    #[inline]
    pub fn get_mut(&mut self, id: Id<T>) -> Option<&mut Node<T>> {
        self.pool.get_mut(id.inner)
    }
}

impl<T> std::ops::Index<Id<T>> for NodeArray<T> {
    type Output = Node<T>;
    #[inline]
    fn index(&self, idx: Id<T>) -> &Self::Output {
        self.get(idx).unwrap()
    }
}

impl<T> std::ops::IndexMut<Id<T>> for NodeArray<T> {
    #[inline]
    fn index_mut(&mut self, idx: Id<T>) -> &mut Self::Output {
        self.get_mut(idx).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use super::*;

    #[test]
    fn smoke_test() {
        let mut array = NodeArray::new();

        let zero = array.add(0u32);
        let one = array.add(1);

        assert_eq!(**array.get(zero).unwrap(), 0);
        assert_eq!(*array[one], 1);

        **array.get_mut(one).unwrap() = 2;
        assert_eq!(*array[one], 2);

        let three = array.add(3);
        assert_eq!(array.len(), 3);

        assert!(array.contains(zero));
        assert!(array.contains(one));
        assert!(array.contains(three));

        let elems: HashSet<_> = array.iter().map(|(_, node)| **node).collect();
        assert_eq!(elems.len(), 3);
        assert!(elems.contains(&0));
        assert!(elems.contains(&2));
        assert!(elems.contains(&3));

        array.remove(three);
        assert_eq!(array.len(), 2);
        assert!(!array.contains(three));
    }
}
