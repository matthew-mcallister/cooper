use std::ptr;
use std::sync::Arc;

use base::{Sentinel, impl_bin_ops};
use derive_more::*;
use vk::traits::*;

use crate::*;

// Sorry if this causes more confusion than good
use self::{
    DescriptorCounts as Counts, DescriptorPool as Pool,
    DescriptorSet as Set, DescriptorSetLayout as Layout,
};

#[derive(Clone, Copy, Debug, Eq, From, Into, PartialEq)]
crate struct DescriptorCounts(na::VectorN<u32, na::U11>);

// TODO: Buffers need an overhaul, as choosing between
// uniform/storage and dynamic/static is an implementation detail
#[derive(Clone, Debug)]
crate struct DescriptorSetLayout {
    device: Arc<Device>,
    inner: vk::DescriptorSetLayout,
    flags: vk::DescriptorSetLayoutCreateFlags,
    bindings: Box<[vk::DescriptorSetLayoutBinding]>,
    counts: Counts,
}

// This alias is appropriate to use anywhere.
crate type SetLayout = DescriptorSetLayout;

#[derive(Debug)]
crate struct DescriptorSet {
    layout: Arc<DescriptorSetLayout>,
    sentinel: Sentinel,
    inner: vk::DescriptorSet,
}

impl Drop for Layout {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe { dt.destroy_descriptor_set_layout(self.inner, ptr::null()); }
    }
}

crate fn pool_sizes(counts: &DescriptorCounts) -> Vec<vk::DescriptorPoolSize> {
    counts.iter()
        .filter_map(|(ty, n)| (n > 0).then_some(
            vk::DescriptorPoolSize { ty, descriptor_count: n }
        ))
        .collect()
}

crate fn count_descriptors(bindings: &[vk::DescriptorSetLayoutBinding]) ->
    Counts
{
    bindings.iter()
        .map(|binding| (binding.descriptor_type, binding.descriptor_count))
        .sum()
}

fn is_valid_type(ty: vk::DescriptorType) -> bool {
    // Not supported yet: texel buffers, dynamic buffers
    [
        vk::DescriptorType::SAMPLER,
        vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        vk::DescriptorType::SAMPLED_IMAGE,
        vk::DescriptorType::STORAGE_IMAGE,
        vk::DescriptorType::UNIFORM_BUFFER,
        vk::DescriptorType::STORAGE_BUFFER,
        vk::DescriptorType::INPUT_ATTACHMENT,
    ].contains(&ty)
}

impl Layout {
    // TODO: Bindless descriptors should be handled by a different type
    crate unsafe fn new(
        device: Arc<Device>,
        flags: vk::DescriptorSetLayoutCreateFlags,
        bindings: &[vk::DescriptorSetLayoutBinding],
    ) -> Self {
        let dt = &*device.table;

        // Validation
        {
            for binding in bindings.iter() {
                assert!(is_valid_type(binding.descriptor_type));
            }
        }

        let create_info = vk::DescriptorSetLayoutCreateInfo {
            flags,
            binding_count: bindings.len() as _,
            p_bindings: bindings.as_ptr(),
            ..Default::default()
        };
        let counts = count_descriptors(bindings);
        let mut inner = vk::null();
        dt.create_descriptor_set_layout(&create_info, ptr::null(), &mut inner)
            .check().unwrap();
        DescriptorSetLayout {
            device,
            inner,
            flags,
            bindings: bindings.into(),
            counts,
        }
    }

    crate unsafe fn from_bindings(
        device: Arc<Device>,
        bindings: &[vk::DescriptorSetLayoutBinding],
    ) -> Self {
        Self::new(device, Default::default(), bindings)
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    crate fn inner(&self) -> vk::DescriptorSetLayout {
        self.inner
    }

    crate fn flags(&self) -> vk::DescriptorSetLayoutCreateFlags {
        self.flags
    }

    crate fn bindings(&self) -> &[vk::DescriptorSetLayoutBinding] {
        &self.bindings
    }

    crate fn counts(&self) -> &Counts {
        &self.counts
    }

    crate fn required_pool_flags(&self) -> vk::DescriptorPoolCreateFlags {
        let mut flags = Default::default();

        let update_after_bind =
            vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL_BIT_EXT;
        if self.flags.contains(update_after_bind) {
            flags |= vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND_BIT_EXT;
        }

        flags
    }
}

fn is_buffer(ty: vk::DescriptorType) -> bool {
    (ty == vk::DescriptorType::UNIFORM_BUFFER)
        | (ty == vk::DescriptorType::STORAGE_BUFFER)
}

fn is_uniform_buffer(ty: vk::DescriptorType) -> bool {
    ty == vk::DescriptorType::UNIFORM_BUFFER
}

fn is_storage_buffer(ty: vk::DescriptorType) -> bool {
    ty == vk::DescriptorType::STORAGE_BUFFER
}

impl DescriptorSet {
    crate fn device(&self) -> &Arc<Device> {
        self.layout.device()
    }

    crate fn inner(&self) -> vk::DescriptorSet {
        self.inner
    }

    crate fn layout(&self) -> &Arc<DescriptorSetLayout> {
        &self.layout
    }

    crate fn write_buffer(
        &mut self,
        binding: u32,
        buffer: impl AsRef<BufferRange>,
    ) {
        self.write_buffers(binding, 0, std::slice::from_ref(buffer.as_ref()));
    }

    /// Writes uniform or storage buffers. Doesn't work with texel
    /// buffers as they require a buffer view object.
    // N.B. direct writes scale poorly compared to update templates.
    crate fn write_buffers(
        &mut self,
        binding: u32,
        first_element: u32,
        buffers: &[impl AsRef<BufferRange>],
    ) {
        let dt = &self.layout.device().table;
        assert_ne!(buffers.len(), 0);

        let layout_binding = &self.layout.bindings()[binding as usize];
        let len = buffers.len() as u32;
        let ty = layout_binding.descriptor_type;

        // Validation
        {
            // N.B. Overrunning writes are actually allowed by the spec
            assert!(first_element + len <= layout_binding.descriptor_count);
            for range in buffers.iter() {
                match range.as_ref().buffer().binding() {
                    BufferBinding::Uniform => assert!(is_uniform_buffer(ty)),
                    BufferBinding::Storage => assert!(is_storage_buffer(ty)),
                    _ => panic!("incompatible descriptor type"),
                };
            }
        }

        let info: Vec<_> = buffers.iter()
            .map(|buffer| buffer.as_ref().descriptor_info())
            .collect();
        let writes = [vk::WriteDescriptorSet {
            dst_set: self.inner(),
            dst_binding: binding,
            dst_array_element: first_element,
            descriptor_count: info.len() as _,
            descriptor_type: ty,
            p_buffer_info: info.as_ptr(),
            ..Default::default()
        }];
        unsafe {
            dt.update_descriptor_sets
                (writes.len() as _, writes.as_ptr(), 0, ptr::null());
        }
    }

    crate unsafe fn write_image(
        &mut self,
        binding: u32,
        view: &Arc<ImageView>,
        sampler: Option<&Arc<Sampler>>,
    ) {
        self.write_images(binding, 0, &[view], try_opt!(&[sampler?][..]));
    }

    /// Writes image to the descriptor set. Combined image/samplers
    /// must specify an array of samplers.
    // TODO: Perhaps should take an iterator
    crate unsafe fn write_images(
        &mut self,
        binding: u32,
        first_element: u32,
        views: &[&Arc<ImageView>],
        samplers: Option<&[&Arc<Sampler>]>,
    ) {
        if let Some(samplers) = samplers {
            assert_eq!(samplers.len(), views.len());
        }

        for (i, &view) in views.iter().enumerate() {
            let sampler = try_opt!(samplers?[i]);
            let elem = first_element + i as u32;
            self.write_image_element(binding, elem, view, sampler);
        }
    }

    unsafe fn write_image_element(
        &mut self,
        binding: u32,
        element: u32,
        view: &Arc<ImageView>,
        sampler: Option<&Arc<Sampler>>,
    ) {
        use vk::DescriptorType as Dt;

        let dt = &self.layout.device().table;
        let layout_binding = &self.layout.bindings()[binding as usize];
        let ty = layout_binding.descriptor_type;

        let sampler = try_opt!(sampler?.inner()).unwrap_or(vk::null());

        // Validation
        {
            assert!(element < layout_binding.descriptor_count);
            let flags = view.image().flags();
            match ty {
                Dt::COMBINED_IMAGE_SAMPLER | Dt::SAMPLED_IMAGE =>
                    assert!(!flags.contains(ImageFlags::NO_SAMPLE)),
                Dt::STORAGE_IMAGE =>
                    assert!(flags.contains(ImageFlags::STORAGE)),
                Dt::INPUT_ATTACHMENT =>
                    assert!(flags.contains(ImageFlags::INPUT_ATTACHMENT)),
                _ => unreachable!(),
            }
            // combined image/sampler <=> sampler not null
            assert_eq!(ty == Dt::COMBINED_IMAGE_SAMPLER, !sampler.is_null());
        }

        // TODO: More sophisticated layout decision making will probably
        // be required eventually.
        let image_layout = match ty {
            Dt::COMBINED_IMAGE_SAMPLER | Dt::SAMPLED_IMAGE =>
                if view.format().is_depth_stencil() {
                    vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL
                } else {
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
                },
            Dt::STORAGE_IMAGE => vk::ImageLayout::GENERAL,
            Dt::INPUT_ATTACHMENT => input_attachment_layout(view.format()),
            _ => unreachable!(),
        };

        let info = [vk::DescriptorImageInfo {
            sampler,
            image_view: view.inner(),
            image_layout,
        }];
        let writes = [vk::WriteDescriptorSet {
            dst_set: self.inner(),
            dst_binding: binding,
            dst_array_element: element,
            descriptor_count: info.len() as _,
            descriptor_type: ty,
            p_image_info: info.as_ptr(),
            ..Default::default()
        }];
        dt.update_descriptor_sets
            (writes.len() as _, writes.as_ptr(), 0, ptr::null());
    }

    crate unsafe fn write_samplers(
        &mut self,
        _binding: u32,
        _first_element: u32,
        _samplers: &[&Arc<Sampler>],
    ) {
        todo!()
    }
}

// Begin boilerplate

impl Default for DescriptorCounts {
    fn default() -> Self {
        Self(na::zero())
    }
}

impl DescriptorCounts {
    crate fn new() -> Self { Default::default() }

    crate fn iter(&self) ->
        impl Iterator<Item = (vk::DescriptorType, u32)> + '_
    {
        self.0.iter().enumerate()
            .map(|(i, v)| (vk::DescriptorType(i as _), *v))
    }

    crate fn iter_mut(&mut self) ->
        impl Iterator<Item = (vk::DescriptorType, &mut u32)>
    {
        self.0.iter_mut().enumerate()
            .map(|(i, v)| (vk::DescriptorType(i as _), v))
    }
}

macro_rules! impl_vec_op {
    ($Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident) => {
        impl std::ops::$OpAssign for DescriptorCounts {
            fn $op_assign(&mut self, other: Self) {
                std::ops::$OpAssign::$op_assign(&mut self.0, other.0);
            }
        }

        impl<'rhs> std::ops::$OpAssign<&'rhs Self> for DescriptorCounts {
            fn $op_assign(&mut self, other: &'rhs Self) {
                std::ops::$OpAssign::$op_assign(&mut self.0, other.0);
            }
        }

        impl_bin_ops!(
            (DescriptorCounts), (DescriptorCounts),
            $Op, $OpAssign, $op, $op_assign,
        );
    }
}

macro_rules! impl_scalar_op {
    ($Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident) => {
        impl std::ops::$OpAssign<u32> for DescriptorCounts {
            fn $op_assign(&mut self, other: u32) {
                std::ops::$OpAssign::<u32>::$op_assign(&mut self.0, other);
            }
        }

        impl<'rhs> std::ops::$OpAssign<&'rhs u32> for DescriptorCounts {
            fn $op_assign(&mut self, other: &'rhs u32) {
                std::ops::$OpAssign::<u32>::$op_assign(&mut self.0, *other);
            }
        }

        impl_bin_ops!(
            (DescriptorCounts), (u32),
            $Op, $OpAssign, $op, $op_assign,
        );
    }
}

impl_vec_op!(Add, AddAssign, add, add_assign);
impl_vec_op!(Sub, SubAssign, sub, sub_assign);
impl_scalar_op!(Mul, MulAssign, mul, mul_assign);
impl_scalar_op!(Div, DivAssign, div, div_assign);

impl std::ops::Index<vk::DescriptorType> for DescriptorCounts {
    type Output = u32;
    fn index(&self, idx: vk::DescriptorType) -> &Self::Output {
        &self.0[idx.0 as usize]
    }
}

impl std::ops::IndexMut<vk::DescriptorType> for DescriptorCounts {
    fn index_mut(&mut self, idx: vk::DescriptorType) -> &mut Self::Output {
        &mut self.0[idx.0 as usize]
    }
}

impl std::iter::Sum<(vk::DescriptorType, u32)> for DescriptorCounts {
    fn sum<I>(iter: I) -> Self
        where I: Iterator<Item = (vk::DescriptorType, u32)>
    {
        let mut counts = DescriptorCounts::default();
        for (k, v) in iter {
            counts[k] += v;
        }
        counts
    }
}

impl std::iter::FromIterator<(vk::DescriptorType, u32)> for DescriptorCounts {
    fn from_iter<I>(iter: I) -> Self
        where I: IntoIterator<Item = (vk::DescriptorType, u32)>
    {
        let mut counts = DescriptorCounts::default();
        for (k, v) in iter {
            counts[k] = v;
        }
        counts
    }
}

// End boilerplate

/// Fixed-size general-purpose descriptor pool.
#[derive(Debug)]
crate struct DescriptorPool {
    device: Arc<Device>,
    inner: vk::DescriptorPool,
    flags: vk::DescriptorPoolCreateFlags,
    // Provides a reference count for safety
    sentinel: Sentinel,
    // Note: limits are not hard but are informative
    max_sets: u32,
    used_sets: u32,
    max_descriptors: Counts,
    used_descriptors: Counts,
}

impl Drop for Pool {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        assert!(!self.sentinel.in_use());
        unsafe { dt.destroy_descriptor_pool(self.inner, ptr::null()); }
    }
}

impl Pool {
    crate unsafe fn new(
        device: Arc<Device>,
        create_info: &vk::DescriptorPoolCreateInfo,
    ) -> Self {
        let dt = &*device.table;

        assert!(create_info.max_sets > 0);
        let pool_sizes = std::slice::from_raw_parts(
            create_info.p_pool_sizes,
            create_info.pool_size_count as usize,
        );
        let max_descriptors = pool_sizes.iter()
            .map(|pool_size| (pool_size.ty, pool_size.descriptor_count))
            .collect();

        let mut inner = vk::null();
        dt.create_descriptor_pool(create_info, ptr::null(), &mut inner)
            .check().unwrap();

        DescriptorPool {
            device,
            inner,
            flags: create_info.flags,
            sentinel: Sentinel::new(),
            max_sets: create_info.max_sets,
            used_sets: 0,
            max_descriptors,
            used_descriptors: Default::default(),
        }
    }

    crate fn can_free(&self) -> bool {
        let flag = vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET_BIT;
        self.flags.contains(flag)
    }

    crate fn max_sets(&self) -> u32 {
        self.max_sets
    }

    crate fn used_sets(&self) -> u32 {
        self.used_sets
    }

    crate fn max_descriptors(&self) -> &Counts {
        &self.max_descriptors
    }

    crate fn used_descriptors(&self) -> &Counts {
        &self.used_descriptors
    }

    crate fn alloc_many(
        &mut self,
        layout: &Arc<DescriptorSetLayout>,
        count: u32,
    ) -> Vec<DescriptorSet> {
        assert!(self.flags.contains(layout.required_pool_flags()));

        self.used_sets += count;
        self.used_descriptors += layout.counts() * count;

        // XXX: use smallvec here
        let dt = &*self.device.table;
        let mut sets = vec![vk::null(); count as usize];
        let layouts = vec![layout.inner(); count as usize];
        let alloc_info = vk::DescriptorSetAllocateInfo {
            descriptor_pool: self.inner,
            descriptor_set_count: count,
            p_set_layouts: layouts.as_ptr(),
            ..Default::default()
        };
        unsafe {
            dt.allocate_descriptor_sets(&alloc_info, sets.as_mut_ptr())
                .check().unwrap();
        }

        sets.into_iter().map(|inner| {
            DescriptorSet {
                layout: layout.clone(),
                sentinel: self.sentinel.clone(),
                inner,
            }
        }).collect()
    }

    crate fn alloc(&mut self, layout: &Arc<DescriptorSetLayout>) -> Set
    {
        self.alloc_many(layout, 1).pop().unwrap()
    }

    crate fn free(&mut self, set: Set) {
        assert!(self.can_free());
        assert_eq!(self.sentinel, set.sentinel);

        self.used_sets -= 1;
        self.used_descriptors -= set.layout.counts();

        let dt = &*self.device.table;
        let sets = std::slice::from_ref(&set.inner);
        unsafe {
            dt.free_descriptor_sets(self.inner, sets.len() as _, sets.as_ptr())
                .check().unwrap();
        }
    }

    crate unsafe fn reset(&mut self) {
        assert!(!self.sentinel.in_use());
        let dt = &*self.device.table;
        dt.reset_descriptor_pool(self.inner, Default::default());
    }
}

/// Returns a reasonable number of descriptor sets and pool sizes for
/// a global descriptor pool.
crate fn global_descriptor_counts() -> (u32, Counts) {
    let max_sets = 0x1_0000;
    let max_descs = [
        (vk::DescriptorType::SAMPLER,                   1 * max_sets),
        (vk::DescriptorType::COMBINED_IMAGE_SAMPLER,    8 * max_sets),
        (vk::DescriptorType::SAMPLED_IMAGE,             8 * max_sets),
        (vk::DescriptorType::STORAGE_IMAGE,             1 * max_sets),
        (vk::DescriptorType::UNIFORM_TEXEL_BUFFER,      1 * max_sets),
        (vk::DescriptorType::STORAGE_TEXEL_BUFFER,      1 * max_sets),
        (vk::DescriptorType::UNIFORM_BUFFER,            1 * max_sets),
        (vk::DescriptorType::STORAGE_BUFFER,            1 * max_sets),
        (vk::DescriptorType::INPUT_ATTACHMENT,          256),
    ].iter().cloned().collect();
    (max_sets, max_descs)
}

crate fn create_global_descriptor_pool(device: Arc<Device>) ->
    DescriptorPool
{
    let (max_sets, max_descriptors) = global_descriptor_counts();
    let pool_sizes = pool_sizes(&max_descriptors);
    let create_info = vk::DescriptorPoolCreateInfo {
        flags: vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET_BIT,
        max_sets,
        pool_size_count: pool_sizes.len() as _,
        p_pool_sizes: pool_sizes.as_ptr(),
        ..Default::default()
    };
    unsafe { DescriptorPool::new(device, &create_info) }
}

#[cfg(test)]
mod tests {
    use vk::traits::*;
    use super::*;

    unsafe fn constant_buffer_layout(device: &Arc<Device>) ->
        Arc<DescriptorSetLayout>
    {
        let device = Arc::clone(device);
        let bindings = [
            vk::DescriptorSetLayoutBinding {
                binding: 0,
                descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::VERTEX_BIT
                    | vk::ShaderStageFlags::FRAGMENT_BIT,
                ..Default::default()
            },
        ];
        DescriptorSetLayout::from_bindings(device, &bindings).into()
    }

    unsafe fn material_layout(device: &Arc<Device>) ->
        Arc<DescriptorSetLayout>
    {
        let device = Arc::clone(device);
        let bindings = [vk::DescriptorSetLayoutBinding {
            binding: 0,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 3,
            stage_flags: vk::ShaderStageFlags::FRAGMENT_BIT,
            ..Default::default()
        }];
        DescriptorSetLayout::from_bindings(device, &bindings).into()
    }

    unsafe fn alloc_test(vars: testing::TestVars) {
        let device = Arc::clone(&vars.swapchain.device);

        let mut pool = create_global_descriptor_pool(Arc::clone(&device));
        let constant_buffer_layout = constant_buffer_layout(&device);
        let material_layout = material_layout(&device);

        let set0 = pool.alloc(&constant_buffer_layout);
        let sets = pool.alloc_many(&material_layout, 3);

        let used = pool.used_descriptors();
        assert_eq!(pool.used_sets(), 4);
        assert_eq!(used[vk::DescriptorType::STORAGE_BUFFER], 1);
        assert_eq!(used[vk::DescriptorType::COMBINED_IMAGE_SAMPLER], 9);

        assert!(!sets.iter().any(|set| set.inner.is_null()));
        assert_ne!(sets[0].inner, sets[1].inner);
        assert_ne!(sets[1].inner, sets[2].inner);
        assert_ne!(sets[2].inner, sets[0].inner);

        pool.free(set0);
        let used = pool.used_descriptors();
        assert_eq!(pool.used_sets(), 3);
        assert_eq!(used[vk::DescriptorType::STORAGE_BUFFER], 0);
    }

    unsafe fn write_test(vars: testing::TestVars) {
        let device = Arc::clone(vars.device());
        let state = SystemState::new(Arc::clone(&device));
        let globals = Globals::new(&state);

        // crate::globals tests possibilities more thoroughly
        let bindings = set_layout_bindings![
            (0, UNIFORM_BUFFER[2]),
            (1, SAMPLED_IMAGE),
            (2, COMBINED_IMAGE_SAMPLER[2]),
        ];
        let layout = Arc::new(SetLayout::from_bindings(device, &bindings));

        let mut descs = state.descriptors.lock();
        let mut desc = descs.alloc(&layout);
        desc.write_buffers(0, 0, &vec![&globals.empty_uniform_buffer; 2]);
        desc.write_image(1, &globals.empty_image_2d, None);
        desc.write_images(
            2,
            0,
            &vec![&globals.empty_image_2d; 2],
            Some(&vec![&globals.empty_sampler; 2]),
        );
    }

    unit::declare_tests![
        alloc_test,
        write_test,
    ];
}

unit::collect_tests![tests];
