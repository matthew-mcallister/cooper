use std::fmt;
use std::ops::{Index, IndexMut};
use std::ptr;

use prelude::*;

use self::Payload::*;

/// Refers to an element of an object pool.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PoolId {
    idx: u32,
    gen: u32,
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
    fn next(self) -> Option<u32> {
        match self {
            Vacant(next) => Some(next),
            _ => None,
        }
    }
}

#[allow(unions_with_drop_fields)]
union PayloadRaw<T> {
    value: T,
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
    payload: PayloadRaw<T>,
}

impl<T> Default for PayloadRaw<T> {
    fn default() -> Self {
        PayloadRaw { next: 0 }
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
        unsafe {
            if let Some(value) = self.value_mut() {
                ptr::drop_in_place(value as _);
            }
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
            payload: PayloadRaw { value },
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

    // TODO: check if this skips copying the payload when T is not Drop
    #[inline]
    fn put(&mut self, payload: Payload<T>) {
        self.swap(payload);
    }

    fn swap(&mut self, payload: Payload<T>) -> Payload<T> {
        unsafe {
            let result = self.copy_payload();
            match payload {
                Occupied(value) => {
                    self.props.set_occupied(true);
                    self.payload.value = value;
                },
                Vacant(next) => {
                    self.props.set_occupied(false);
                    self.payload.next = next;
                },
            }
            result
        }
    }

    // Unsafe if T is not Copy
    #[inline]
    unsafe fn copy_payload(&self) -> Payload<T> {
        if self.props.occupied() {
            Occupied(ptr::read(&self.payload.value as _))
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
    size: u32,
    next: u32,
}

impl<T> Default for Pool<T> {
    fn default() -> Self {
        Pool {
            slots: Default::default(),
            size: Default::default(),
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
            size: 0,
        }
    }

    pub fn size(&self) -> u32 {
        self.size
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

    // TODO: Add might be made thread safe (via CAS) except possibly for
    // the case where the pool is at capacity.
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
            let new_next = slot.swap(Occupied(value)).next().unwrap();
            self.next = new_next;
        }
        self.size += 1;
        PoolId { idx, gen }
    }

    pub fn remove(&mut self, id: PoolId) {
        let _: Option<_> = try {
            let next = self.next;
            let slot = self.get_slot_mut(id)?;
            slot.put(Vacant(next));
            slot.invalidate();
            self.next = id.idx;
            self.size -= 1;
        };
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

    pub fn iter_mut(&self) -> impl Iterator<Item = (PoolId, &T)> {
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

    // TODO: More tests, obviously
    #[test]
    fn smoke_tests() {
        let mut pool: Pool<u32> = Pool::new();

        // Can't get anything if pool is empty
        let id = PoolId { idx: 0, gen: 0 };
        assert!(pool.get(id).is_none());
        assert!(!pool.contains(id));
        assert_eq!(pool.size(), 0);

        // Can push and read back
        let id = pool.add(24);
        assert_eq!(pool[id], 24);
        assert!(pool.contains(id));
        assert_eq!(pool.size(), 1);
        let id2 = pool.add(42);
        assert_eq!(pool[id], 24);
        assert_eq!(pool[id2], 42);
        assert_eq!(pool.size(), 2);

        // Can successfully remove
        pool.remove(id);
        assert!(!pool.contains(id));
        assert_eq!(pool.size(), 1);
        pool.remove(id2);
        assert!(!pool.contains(id));
        assert!(!pool.contains(id2));
        assert_eq!(pool.size(), 0);

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