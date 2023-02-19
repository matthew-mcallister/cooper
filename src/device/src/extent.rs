use derive_more::*;
use math::{Ivector3, Uvector2, Uvector3};

macro_rules! impl_conversion {
    ($from:ty => $via:ty => $into:ty) => {
        impl From<$from> for $into {
            #[inline]
            fn from(from: $from) -> Self {
                let tmp: $via = from.into();
                tmp.into()
            }
        }
    };
}

#[derive(
    Clone, Constructor, Copy, Debug, Default, Eq, From, Hash, Into, Mul, MulAssign, PartialEq,
)]
pub struct Extent2D {
    pub width: u32,
    pub height: u32,
}

#[derive(
    Clone, Constructor, Copy, Debug, Default, Eq, From, Hash, Into, Mul, MulAssign, PartialEq,
)]
pub struct Extent3D {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
}

impl_conversion!(Uvector2 => [u32; 2] => Extent2D);
impl_conversion!(vk::Extent2D => (u32, u32) => Extent2D);
impl_conversion!(Extent2D => [u32; 2] => Uvector2);
impl_conversion!(Extent2D => (u32, u32) => vk::Extent2D);

impl_conversion!(Uvector3 => [u32; 3] => Extent3D);
impl_conversion!(vk::Extent3D => (u32, u32, u32) => Extent3D);
impl_conversion!((u32, u32) => Extent2D => Extent3D);
impl_conversion!(Extent3D => [u32; 3] => Uvector3);
impl_conversion!(Extent3D => (u32, u32, u32) => vk::Extent3D);

impl From<[u32; 2]> for Extent2D {
    fn from([width, height]: [u32; 2]) -> Self {
        Self { width, height }
    }
}

impl From<Extent2D> for [u32; 2] {
    fn from(Extent2D { width, height }: Extent2D) -> Self {
        [width, height]
    }
}

impl From<[u32; 3]> for Extent3D {
    fn from([width, height, depth]: [u32; 3]) -> Self {
        Self {
            width,
            height,
            depth,
        }
    }
}

impl From<Extent3D> for [u32; 3] {
    fn from(
        Extent3D {
            width,
            height,
            depth,
        }: Extent3D,
    ) -> Self {
        [width, height, depth]
    }
}

impl From<Extent2D> for Extent3D {
    #[inline]
    fn from(extent: Extent2D) -> Self {
        Self::new(extent.width, extent.height, 1)
    }
}

impl AsRef<[u32; 2]> for Extent2D {
    #[inline]
    fn as_ref(&self) -> &[u32; 2] {
        unsafe { &*(self as *const _ as *const _) }
    }
}

impl AsRef<[u32; 3]> for Extent3D {
    #[inline]
    fn as_ref(&self) -> &[u32; 3] {
        unsafe { &*(self as *const _ as *const _) }
    }
}

impl Extent2D {
    #[inline]
    pub fn to_vec(self) -> Uvector2 {
        self.into()
    }

    #[inline]
    pub fn as_array(&self) -> &[u32; 2] {
        self.as_ref()
    }
}

impl Extent3D {
    #[inline]
    pub fn to_vec(self) -> Uvector3 {
        self.into()
    }

    #[inline]
    pub fn as_array(&self) -> &[u32; 3] {
        self.as_ref()
    }

    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &u32> {
        self.as_array().iter()
    }

    #[inline]
    pub fn to_2d(self) -> Extent2D {
        Extent2D::new(self.width, self.height)
    }

    #[inline]
    pub fn contains_extent(&self, offset: Ivector3, extent: Self) -> bool {
        fn check_range_overflow(offs: i32, len: u32, max: u32) -> bool {
            (offs >= 0) & (len <= max) & ((offs as u32) <= max.wrapping_sub(len))
        }
        self.iter()
            .zip(offset.iter())
            .zip(extent.iter())
            .all(|((&max, &offs), &len)| check_range_overflow(offs, len, max))
    }

    #[inline]
    pub fn texel_count(&self) -> vk::DeviceSize {
        self.width as vk::DeviceSize * self.height as vk::DeviceSize * self.depth as vk::DeviceSize
    }

    #[inline]
    pub fn mip_level(&self, level: u32) -> Self {
        self.to_vec().map(|x| std::cmp::max(1, x >> level)).into()
    }

    #[inline]
    pub fn mip_levels(&self) -> u32 {
        let max_dim = self.iter().max().unwrap();
        32 - max_dim.leading_zeros()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing;
    use math::ivec3;

    unsafe fn contains(_: testing::TestVars) {
        let ex = Extent3D::new;
        let scrn = ex(1920, 1080, 1);
        assert!(scrn.contains_extent(ivec3(0, 0, 0), scrn));
        assert!(scrn.contains_extent(ivec3(1920, 1080, 1), ex(0, 0, 0)));
        assert!(scrn.contains_extent(ivec3(1, 2, 0), ex(3, 4, 1)));
        assert!(!scrn.contains_extent(ivec3(0, 0, 0), ex(1920, 1080, 2)));
        assert!(!scrn.contains_extent(ivec3(-1, -1, -1), ex(1, 1, 1)));
        assert!(!scrn.contains_extent(ivec3(1920, 1080, 1), ex(1, 1, 1)));
        // Even though one of the dimensions is 0, the subregion cannot
        // overrun the extent dimensions.
        assert!(!scrn.contains_extent(ivec3(1920, 1080, 1), ex(0, 0, 1)));
    }

    unsafe fn mip_levels(_: testing::TestVars) {
        let ex = Extent3D::new;
        let even = ex(32, 32, 2);
        assert_eq!(even.mip_level(0), even);
        assert_eq!(even.mip_level(1), ex(16, 16, 1));
        assert_eq!(even.mip_level(2), ex(8, 8, 1));
        assert_eq!(even.mip_level(5), ex(1, 1, 1));
        assert_eq!(even.mip_level(6), ex(1, 1, 1));
        assert_eq!(even.mip_levels(), 6);
        assert_eq!(even.texel_count(), 32 * 32 * 2);
        assert_eq!(even.mip_level(1).texel_count(), even.texel_count() / 8);
        let odd = ex(35, 3, 11);
        assert_eq!(odd.mip_level(0), odd);
        assert_eq!(odd.mip_level(1), ex(17, 1, 5));
        assert_eq!(odd.mip_level(2), ex(8, 1, 2));
        assert_eq!(odd.mip_level(odd.mip_levels()), ex(1, 1, 1));
        assert_eq!(odd.mip_levels(), 6);
        assert_eq!(odd.texel_count(), 35 * 3 * 11);
        assert_eq!(odd.mip_level(1).texel_count(), 17 * 1 * 5);
    }
}
