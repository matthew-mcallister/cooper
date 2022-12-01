use std::fmt;
use std::mem::ManuallyDrop;
use std::ops::{Index, IndexMut};

use prelude::*;

use self::Payload::*;

/// Refers to an element of an object pool.
// TODO: sizeof(Option<PoolId>) should be 8
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PoolId {
    pub(crate) idx: u32,
    pub(crate) gen: u32,
}

bitfield! {
    #[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
    struct SlotProps(u32) {
        // Incrementing this invalidates outstanding references.
        {
            getter: gen,
            setter: set_gen,
            type: u32,
            bits: (0, 31),
        },
        // This bit is necessary to properly call destructors.
        {
            getter: occupied,
            setter: set_occupied,
            type: bool,
            bit: 31,
        },
    }
}

#[derive(Debug)]
enum Payload<T> {
    Occupied(T),
    Vacant(u32),
}

impl<T> Payload<T> {
    #[inline]
    fn is_occupied(&self) -> bool {
        self.as_ref().value().is_some()
    }

    #[inline]
    fn value(self) -> Option<T> {
        match self {
            Occupied(value) => Some(value),
            _ => None,
        }
    }

    #[inline]
    fn next(self) -> Option<u32> {
        match self {
            Vacant(next) => Some(next),
            _ => None,
        }
    }

    #[inline]
    fn into_raw(self) -> RawPayload<T> {
        match self {
            Occupied(value) => RawPayload { value: ManuallyDrop::new(value) },
            Vacant(next) => RawPayload { next },
        }
    }

    #[inline]
    fn as_ref(&self) -> Payload<&T> {
        match self {
            Occupied(ref value) => Occupied(value),
            Vacant(next) => Vacant(*next),
        }
    }
}

union RawPayload<T> {
    value: ManuallyDrop<T>,
    // Free list pointer
    next: u32,
}

// Nearly equivalent to this struct:
// ```
// struct Slot<T> {
//     gen: u32,
//     payload: Payload<T>,
// }
// ```
// The discriminant of `Payload` is packed into the top bit of `gen`.
struct Slot<T> {
    props: SlotProps,
    payload: RawPayload<T>,
}

impl<T> Default for RawPayload<T> {
    fn default() -> Self {
        RawPayload { next: 0 }
    }
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Slot {
            props: Default::default(),
            payload: Default::default(),
        }
    }
}

impl<T> Drop for Slot<T> {
    #[inline]
    fn drop(&mut self) {
        if self.props.occupied() {
            unsafe { ManuallyDrop::drop(&mut self.payload.value); }
        }
    }
}

impl<T> fmt::Debug for Slot<T>
    where T: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Slot")
            .field("gen", &self.props.gen())
            .field("payload", &self.payload())
            .finish()
    }
}

impl<T> Slot<T> {
    #[inline]
    fn new(value: T) -> Self {
        let mut props: SlotProps = Default::default();
        props.set_occupied(true);
        Slot {
            props,
            payload: RawPayload { value: ManuallyDrop::new(value) },
        }
    }

    #[inline]
    fn gen(&self) -> u32 {
        self.props.gen()
    }

    #[inline]
    fn value(&self) -> Option<&T> {
        opt(self.props.occupied())?;
        unsafe { Some(&self.payload.value) }
    }

    #[inline]
    fn value_mut(&mut self) -> Option<&mut T> {
        opt(self.props.occupied())?;
        unsafe { Some(&mut self.payload.value) }
    }

    #[inline]
    fn replace(&mut self, other: Payload<T>) -> Payload<T> {
        unsafe {
            let result = self.take_payload();
            self.props.set_occupied(other.is_occupied());
            self.payload = other.into_raw();
            result
        }
    }

    #[inline]
    unsafe fn take_payload(&mut self) -> Payload<T> {
        if self.props.occupied() {
            Occupied(ManuallyDrop::take(&mut self.payload.value as _))
        } else {
            Vacant(self.payload.next)
        }
    }

    #[inline]
    fn payload(&self) -> Payload<&T> {
        unsafe {
            if self.props.occupied() {
                Occupied(&self.payload.value)
            } else {
                Vacant(self.payload.next)
            }
        }
    }

    #[inline]
    fn invalidate(&mut self) {
        let gen = self.props.gen() + 1;
        self.props.set_gen(gen);
    }
}

/// A safe storage structure that allows elements to be quickly added
/// and removed while retaining a stable identifier.
#[derive(Debug)]
pub struct Pool<T> {
    slots: Vec<Slot<T>>,
    len: u32,
    next: u32,
}

impl<T> Default for Pool<T> {
    fn default() -> Self {
        Pool {
            slots: Default::default(),
            len: Default::default(),
            next: Default::default(),
        }
    }
}

impl<T> Pool<T> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_capacity(size: u32) -> Self {
        Pool {
            slots: Vec::with_capacity(size as _),
            next: 0,
            len: 0,
        }
    }

    pub fn len(&self) -> u32 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn get_slot(&self, id: PoolId) -> Option<&Slot<T>> {
        let slot = self.slots.get(id.idx as usize)?;
        opt(slot.gen() == id.gen)?;
        Some(slot)
    }

    fn get_slot_mut(&mut self, id: PoolId) -> Option<&mut Slot<T>> {
        let slot = self.slots.get_mut(id.idx as usize)?;
        opt(slot.gen() == id.gen)?;
        Some(slot)
    }

    pub fn get(&self, id: PoolId) -> Option<&T> {
        self.get_slot(id)?.value()
    }

    pub fn get_mut(&mut self, id: PoolId) -> Option<&mut T> {
        self.get_slot_mut(id)?.value_mut()
    }

    // TODO: Add might be made thread safe (via CAS or probing) except
    // possibly for the case where the pool is at capacity.
    //
    // My best idea yet is a thread-safe "try_add" method that may fail
    // under racy circumstances; the caller can fall back on locking.
    pub fn add(&mut self, value: T) -> PoolId {
        let (idx, gen);
        assert!(self.slots.len() < u32::max_value() as usize);
        if self.next == self.slots.len() as u32 {
            idx = self.slots.len() as _;
            gen = 0;
            self.slots.push(Slot::new(value));
            self.next = self.slots.len() as _;
        } else {
            idx = self.next;
            let slot = &mut self.slots[idx as usize];
            gen = slot.gen();
            let new_next = slot.replace(Occupied(value)).next().unwrap();
            self.next = new_next;
        }
        self.len += 1;
        PoolId { idx, gen }
    }

    pub fn remove(&mut self, id: PoolId) -> Option<T> {
        let next = self.next;
        let slot = self.get_slot_mut(id)?;
        let value = slot.replace(Vacant(next)).value().unwrap();
        slot.invalidate();
        self.next = id.idx;
        self.len -= 1;
        Some(value)
    }

    pub fn contains(&self, id: PoolId) -> bool {
        self.get_slot(id).is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = (PoolId, &T)> {
        self.slots.iter()
            .enumerate()
            .filter_map(|(idx, slot)| {
                let value = slot.value()?;
                let id = PoolId {
                    idx: idx as u32,
                    gen: slot.gen(),
                };
                Some((id, value))
            })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (PoolId, &mut T)> {
        self.slots.iter_mut()
            .enumerate()
            .filter_map(|(idx, slot)| {
                let id = PoolId {
                    idx: idx as u32,
                    gen: slot.gen(),
                };
                let value = slot.value_mut()?;
                Some((id, value))
            })
    }
}

impl<T> Index<PoolId> for Pool<T> {
    type Output = T;
    fn index(&self, index: PoolId) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl<T> IndexMut<PoolId> for Pool<T> {
    fn index_mut(&mut self, index: PoolId) -> &mut Self::Output {
        self.get_mut(index).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn smoke_tests() {
        let mut pool: Pool<u32> = Pool::new();

        // Can't get anything if pool is empty
        let id = PoolId { idx: 0, gen: 0 };
        assert!(pool.get(id).is_none());
        assert!(!pool.contains(id));
        assert_eq!(pool.len(), 0);

        // Can push and read back
        let id = pool.add(24);
        assert_eq!(pool[id], 24);
        assert!(pool.contains(id));
        assert_eq!(pool.len(), 1);
        let id2 = pool.add(42);
        assert_eq!(pool[id], 24);
        assert_eq!(pool[id2], 42);
        assert_eq!(pool.len(), 2);

        // Can successfully remove
        pool.remove(id);
        assert!(!pool.contains(id));
        assert_eq!(pool.len(), 1);
        pool.remove(id2);
        assert!(!pool.contains(id));
        assert!(!pool.contains(id2));
        assert_eq!(pool.len(), 0);

        // Can mutate
        let id = pool.add(7);
        pool[id] -= 1;
        assert_eq!(pool[id], 6);

        // Can iterate
        let id2 = pool.add(12);
        let exp: HashSet<_> = [(id, 6), (id2, 12)].iter().cloned().collect();
        let got: HashSet<_> = pool.iter()
            .map(|(id, &value)| (id, value))
            .collect();
        assert_eq!(exp, got);
    }
}
