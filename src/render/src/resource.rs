//! This module implements a graph for automatic resource management.
use std::marker::PhantomData;
use std::ptr;
use std::sync::Arc;

use crate::*;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Id<T> {
    gen: u32, // TODO: not needed in production
    idx: u32,
    _marker: PhantomData<*const T>,
}

impl<T> Id<T> {
    fn new(gen: u32, idx: u32) -> Self {
        Id { gen, idx, _marker: PhantomData }
    }
}

crate trait Resource: Copy {
    #[allow(unused_variables)]
    fn insert_refs(self, graph: &mut ResourceGraph) {}
    unsafe fn destroy(self, graph: &mut ResourceGraph);
}

#[derive(Clone, Copy, Debug)]
pub struct Sampler {
    sampler: vk::Sampler,
}

impl Resource for Sampler {
    unsafe fn destroy(self, graph: &mut ResourceGraph) {
        graph.dt.destroy_sampler(self.sampler, ptr::null());
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Image {
    view: vk::ImageView,
    image: vk::Image,
    memory: CommonAlloc,
}

impl Resource for Image {
    unsafe fn destroy(self, graph: &mut ResourceGraph) {
        graph.dt.destroy_image_view(self.view, ptr::null());
        graph.dt.destroy_image(self.image, ptr::null());
        graph.image_alloc.free(&self.memory);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SampledImage {
    sampler: Id<Sampler>,
    image: Id<Image>,
}

// TODO: Do "resources" that consist entirely of references really need
// to exist? Could be useful for material sorting.
#[derive(Clone, Copy, Debug)]
pub struct PbrMaterial {
    albedo: SampledImage,
    normal: SampledImage,
    surface: SampledImage,
}

impl Resource for PbrMaterial {
    fn insert_refs(self, graph: &mut ResourceGraph) {
        graph.add_ref(self.albedo.sampler);
        graph.add_ref(self.albedo.image);
        graph.add_ref(self.normal.sampler);
        graph.add_ref(self.normal.image);
        graph.add_ref(self.surface.sampler);
        graph.add_ref(self.surface.image);
    }

    unsafe fn destroy(self, graph: &mut ResourceGraph) {
        graph.sub_ref(self.albedo.sampler);
        graph.sub_ref(self.albedo.image);
        graph.sub_ref(self.normal.sampler);
        graph.sub_ref(self.normal.image);
        graph.sub_ref(self.surface.sampler);
        graph.sub_ref(self.surface.image);
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Material {
    Pbr(PbrMaterial),
}

impl Resource for Material {
    fn insert_refs(self, graph: &mut ResourceGraph) {
        match self {
            Material::Pbr(m) => m.insert_refs(graph),
        }
    }

    unsafe fn destroy(self, graph: &mut ResourceGraph) {
        match self {
            Material::Pbr(m) => m.destroy(graph),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Mesh {
    attrs: VertexAttrs,
    view: vk::BufferView,
    buffer: vk::Buffer,
    memory: CommonAlloc,
}

impl Resource for Mesh {
    unsafe fn destroy(self, graph: &mut ResourceGraph) {
        graph.dt.destroy_buffer_view(self.view, ptr::null());
        graph.dt.destroy_buffer(self.buffer, ptr::null());
        graph.buffer_alloc.free(&self.memory);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Surface {
    mesh: Id<Mesh>,
    material: Id<Material>,
}

impl Resource for Surface {
    unsafe fn destroy(self, graph: &mut ResourceGraph) {
        graph.sub_ref(self.mesh);
        graph.sub_ref(self.material);
    }
}

/// A poor attempt at data structure programming. Basically a substitute
/// for the venerable linked list with reference-counting.
///
/// The `Copy` trait bound is only there to avoid the question of how
/// to ensure destructors are never run.
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

impl std::fmt::Debug for SlotMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "SlotMeta {{ *union* }}")
    }
}

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

#[derive(Debug)]
crate struct ResourceGraph {
    dt: Arc<vkl::DeviceTable>,
    buffer_alloc: MemoryPool,
    image_alloc: MemoryPool,
    samplers: Arena<Sampler>,
    images: Arena<Image>,
    materials: Arena<Material>,
    meshes: Arena<Mesh>,
    surfaces: Arena<Surface>,
}

crate trait HasResource<T: Resource> {
    fn table(&self) -> &Arena<T>;
    fn table_mut(&mut self) -> &mut Arena<T>;
}

macro_rules! impl_resources {
    ($(($type:ident, $table:ident),)*) => {
        $(
            impl HasResource<$type> for ResourceGraph {
                fn table(&self) -> &Arena<$type> {
                    &self.$table
                }
                fn table_mut(&mut self) -> &mut Arena<$type> {
                    &mut self.$table
                }
            }
        )*
    }
}

impl_resources! {
    (Sampler, samplers),
    (Image, images),
    (Material, materials),
    (Mesh, meshes),
    (Surface, surfaces),
}

impl ResourceGraph {
    crate fn insert<T>(&mut self, val: T) -> Id<T>
    where
        T: Resource,
        Self: HasResource<T>,
    {
        val.insert_refs(self);
        self.table_mut().insert(val)
    }

    crate fn add_ref<T>(&mut self, id: Id<T>)
    where
        T: Resource,
        Self: HasResource<T>,
    {
        self.table_mut().add_ref(id);
    }

    crate fn sub_ref<T>(&mut self, id: Id<T>)
    where
        T: Resource,
        Self: HasResource<T>,
    {
        if let Some(obj) = self.table_mut().sub_ref(id) {
            unsafe { obj.destroy(self); }
        }
    }

    crate fn get<T>(&self, id: Id<T>) -> &T
    where
        T: Resource,
        Self: HasResource<T>,
    {
        self.table().get(id)
    }

    crate fn get_mut<T>(&mut self, id: Id<T>) -> &mut T
    where
        T: Resource,
        Self: HasResource<T>,
    {
        self.table_mut().get_mut(id)
    }
}
