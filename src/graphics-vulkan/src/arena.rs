#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Id<T> {
    gen: u32, // TODO: not needed in production
    idx: u32,
    _marker: PhantomData<*const T>,
}

impl<T> Id<T> {
    crate fn new(gen: u32, idx: u32) -> Self {
        Id { gen, idx, _marker: PhantomData }
    }
}

/// The `Copy` trait bound is only there to avoid the question of how
/// to ensure destructors are not run inappropriately.
#[derive(Debug)]
crate struct Arena<T: Copy> {
    slots: Vec<Slot<T>>,
    counter: u32,
    next: u32,
}

#[derive(Debug)]
struct Slot<T: Copy> {
    ref_count: u32,
    meta: SlotMeta,
    data: T,
}

#[derive(Clone, Copy)]
union SlotMeta {
    next: u32,
    gen: u32,
}

impl_debug_union!(SlotMeta);

impl<T: Copy> Slot<T> {
    fn validate_ref(&self, id: Id<T>) {
        assert_ne!(self.ref_count, 0);
        assert_eq!(id.gen, unsafe { self.meta.gen });
    }
}

impl<T: Copy> Arena<T> {
    crate fn new() -> Self {
        Arena::with_capacity(0)
    }

    crate fn with_capacity(capacity: u32) -> Self {
        Arena {
            slots: Vec::with_capacity(capacity as _),
            counter: 0,
            next: 0,
        }
    }

    crate fn insert(&mut self, val: T) -> Id<T> {
        let gen = self.counter;
        self.counter += 1;
        if self.next as usize >= self.slots.len() {
            self.slots.push(Slot {
                ref_count: 1,
                meta: SlotMeta { gen },
                data: val,
            });
            self.next = self.slots.len() as _;
            Id::new(gen, self.slots.len() as _)
        } else {
            let idx = self.next;
            let slot: &mut Slot<T> = &mut self.slots[idx as usize];
            debug_assert_eq!(slot.ref_count, 0);
            self.next = unsafe { slot.meta.next };
            *slot = Slot {
                ref_count: 1,
                meta: SlotMeta { gen },
                data: val,
            };
            Id::new(gen, idx)
        }
    }

    crate fn add_ref(&mut self, id: Id<T>) {
        let slot = &mut self.slots[id.idx as usize];
        slot.validate_ref(id);
        slot.ref_count += 1;
    }

    crate fn sub_ref(&mut self, id: Id<T>) -> Option<T> {
        let slot = &mut self.slots[id.idx as usize];
        slot.validate_ref(id);
        slot.ref_count -= 1;
        if slot.ref_count == 0 {
            slot.meta.next = self.next;
            self.next = id.idx;
            Some(slot.data)
        } else { None }
    }

    crate fn get(&self, id: Id<T>) -> &'_ T {
        let slot = &self.slots[id.idx as usize];
        slot.validate_ref(id);
        &slot.data
    }

    crate fn get_mut(&mut self, id: Id<T>) -> &mut T {
        let slot = &mut self.slots[id.idx as usize];
        slot.validate_ref(id);
        &mut slot.data
   }
}
