use std::borrow::Cow;
use std::ptr;
use std::sync::Arc;

use derivative::*;

use crate::*;

#[derive(Debug)]
pub struct Sampler {
    device: Arc<Device>,
    desc: SamplerDesc,
    inner: vk::Sampler,
}

#[derive(Clone, Copy, Debug, Default, Derivative)]
#[derivative(Hash, PartialEq)]
pub struct SamplerDesc {
    pub mag_filter: Filter,
    pub min_filter: Filter,
    pub mipmap_mode: SamplerMipmapMode,
    pub address_mode_u: SamplerAddressMode,
    pub address_mode_v: SamplerAddressMode,
    pub address_mode_w: SamplerAddressMode,
    pub anisotropy_level: AnisotropyLevel,
    #[derivative(Hash(hash_with = "byte_hash"))]
    #[derivative(PartialEq(compare_with = "byte_eq"))]
    // TODO: Ideally replace with non-floating-point value
    pub mip_lod_bias: f32,
    pub border_color: BorderColor,
    pub unnormalized_coordinates: bool,
}

impl Eq for SamplerDesc {}

wrap_vk_enum! {
    #[derive(Derivative)]
    #[derivative(Default)]
    pub enum Filter {
        #[derivative(Default)]
        Nearest = NEAREST,
        Linear = LINEAR,
    }
}

wrap_vk_enum! {
    #[derive(Derivative)]
    #[derivative(Default)]
    pub enum SamplerMipmapMode {
        #[derivative(Default)]
        Nearest = NEAREST,
        Linear = LINEAR,
    }
}

wrap_vk_enum! {
    #[derive(Derivative)]
    #[derivative(Default)]
    pub enum SamplerAddressMode {
        #[derivative(Default)]
        Repeat = REPEAT,
        MirroredRepeat = MIRRORED_REPEAT,
        ClampToEdge = CLAMP_TO_EDGE,
        ClampToBorder = CLAMP_TO_BORDER,
        MirrorClampToEdge = MIRROR_CLAMP_TO_EDGE,
    }
}

#[derive(Clone, Copy, Debug, Derivative, Eq, Hash, PartialEq)]
#[derivative(Default)]
pub enum AnisotropyLevel {
    #[derivative(Default)]
    One,
    Two,
    Four,
    Eight,
    Sixteen,
}

wrap_vk_enum! {
    #[derive(Derivative)]
    #[derivative(Default)]
    pub enum BorderColor {
        #[derivative(Default)]
        TransparentBlack = FLOAT_TRANSPARENT_BLACK,
        OpaqueBlack = FLOAT_OPAQUE_BLACK,
        OpaqueWhite = FLOAT_OPAQUE_WHITE,
        // TODO: Int colors?
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe { dt.destroy_sampler(self.inner, ptr::null()); }
    }
}

impl_device_derived!(Sampler);

impl Sampler {
    pub fn new(device: Arc<Device>, desc: SamplerDesc) -> Self { unsafe {
        let dt = &*device.table;

        assert!((0.0 <= desc.mip_lod_bias) & (desc.mip_lod_bias <= 15.0));
        debug_assert_eq!(device.features().sampler_anisotropy, vk::TRUE);
        assert!(device.limits().max_sampler_lod_bias >= 15.0);
        assert!(device.limits().max_sampler_anisotropy >= 16.0);

        let create_info = vk::SamplerCreateInfo {
            mag_filter: desc.mag_filter.into(),
            min_filter: desc.min_filter.into(),
            mipmap_mode: desc.mipmap_mode.into(),
            address_mode_u: desc.address_mode_u.into(),
            address_mode_v: desc.address_mode_v.into(),
            address_mode_w: desc.address_mode_w.into(),
            mip_lod_bias: desc.mip_lod_bias,
            anisotropy_enable: bool32(desc.anisotropy_level.is_anisotropic()),
            max_anisotropy: desc.anisotropy_level.into(),
            // TODO: Not really sure when you'd want to change these
            // limits. Maybe for sparse textures with missing mips?
            min_lod: 0.0,
            max_lod: 1024.0,
            border_color: desc.border_color.into(),
            unnormalized_coordinates: bool32(desc.unnormalized_coordinates),
            ..Default::default()
        };
        let mut sampler = vk::null();
        dt.create_sampler(&create_info, ptr::null(), &mut sampler)
            .check().unwrap();

        Self {
            device,
            desc,
            inner: sampler,
        }
    } }

    #[inline]
    pub fn inner(&self) -> vk::Sampler {
        self.inner
    }

    #[inline]
    pub fn desc(&self) -> &SamplerDesc {
        &self.desc
    }
}

impl AnisotropyLevel {
    fn is_anisotropic(self) -> bool {
        self != Self::One
    }
}

impl From<AnisotropyLevel> for f32 {
    fn from(level: AnisotropyLevel) -> Self {
        match level {
            AnisotropyLevel::One => 1.0,
            AnisotropyLevel::Two => 2.0,
            AnisotropyLevel::Four => 4.0,
            AnisotropyLevel::Eight => 8.0,
            AnisotropyLevel::Sixteen => 16.0,
        }
    }
}

/// A multithreaded object storage system for sampler objects based on
/// the pipeline creation cache.
#[derive(Debug)]
pub struct SamplerCache {
    device: Arc<Device>,
    inner: StagedCache<SamplerDesc, Arc<Sampler>>,
}

impl SamplerCache {
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            device,
            inner: Default::default(),
        }
    }

    pub fn commit(&mut self) {
        self.inner.commit();
    }

    pub fn get_committed(&self, desc: &SamplerDesc) ->
        Option<&Arc<Sampler>>
    {
        self.inner.get_committed(desc)
    }

    pub fn get_or_create(&self, desc: &SamplerDesc) -> Cow<Arc<Sampler>> {
        self.inner.get_or_insert_with(desc, || Arc::new(Sampler::new(
            Arc::clone(&self.device),
            *desc,
        )))
    }

    pub fn get_or_create_committed(&mut self, desc: &SamplerDesc) ->
        &Arc<Sampler>
    {
        let device = &self.device;
        self.inner.get_or_insert_committed_with(desc, || Arc::new(Sampler::new(
            Arc::clone(&device),
            *desc,
        )))
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use super::*;

    fn basic_sampler_desc() -> SamplerDesc {
        SamplerDesc {
            mag_filter: Filter::Linear,
            min_filter: Filter::Linear,
            mipmap_mode: SamplerMipmapMode::Linear,
            anisotropy_level: AnisotropyLevel::Sixteen,
            mip_lod_bias: 1.0,
            ..Default::default()
        }
    }

    fn creation_test(vars: testing::TestVars) {
        let desc = basic_sampler_desc();
        let _sampler = Sampler::new(Arc::clone(vars.device()), desc);
    }

    fn cache_test(vars: testing::TestVars) {
        let mut cache = SamplerCache::new(Arc::clone(vars.device()));

        let desc = basic_sampler_desc();
        let _s0 = Arc::clone(&cache.get_or_create(&desc));

        let desc = SamplerDesc {
            mipmap_mode: SamplerMipmapMode::Linear,
            anisotropy_level: AnisotropyLevel::Sixteen,
            ..Default::default()
        };
        let s1 = Arc::clone(&cache.get_or_create(&desc));

        cache.commit();

        assert!(Arc::ptr_eq(cache.get_committed(&desc).unwrap(), &s1));
    }

    unit::declare_tests![creation_test, cache_test];
}

unit::collect_tests![tests];
