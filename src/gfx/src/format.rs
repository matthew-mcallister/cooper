use crate::*;

macro_rules! impl_format {
    ($($name:ident($vk_format:ident, $($aspect:ident)|*),)*) => {
        /// A selection of the most useful data formats. Keep in mind
        /// that not all devices/drivers support all formats.
        ///
        /// # Naming conventions
        ///
        /// No suffix means `_UNORM`. An `F` means `_SFLOAT`. The size
        /// is not repeated for each component if they all are the same
        /// size.
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        #[allow(non_camel_case_types)]
        crate enum Format {
            $($name,)*
        }

        impl Format {
            // TODO: Should probably implement on vk::Format
            crate fn aspects(self) -> vk::ImageAspectFlags {
                match self {$(
                    Format::$name => $(vk::ImageAspectFlags::$aspect)|*,
                )*}
            }
        }

        impl From<Format> for vk::Format {
            fn from(fmt: Format) -> Self {
                match fmt {
                    $(Format::$name => vk::Format::$vk_format,)*
                }
            }
        }
    }
}

impl_format! {
    R8(R8_UNORM, COLOR_BIT),
    R16F(R16_SFLOAT, COLOR_BIT),
    R32F(R32_SFLOAT, COLOR_BIT),
    RG8(R8G8_UNORM, COLOR_BIT),
    RG16F(R16G16_SFLOAT, COLOR_BIT),
    RG32F(R32G32_SFLOAT, COLOR_BIT),
    RGB8(R8G8B8_UNORM, COLOR_BIT),
    RGB16F(R16G16B16_SFLOAT, COLOR_BIT),
    RGB32F(R32G32B32_SFLOAT, COLOR_BIT),
    RGBA8(R8G8B8A8_UNORM, COLOR_BIT),
    RGBA16F(R16G16B16A16_SFLOAT, COLOR_BIT),
    RGBA32F(R32G32B32A32_SFLOAT, COLOR_BIT),
    BGRA8_SRGB(B8G8R8A8_SRGB, COLOR_BIT),
    D16(D16_UNORM, DEPTH_BIT),
    D32F(D32_SFLOAT, DEPTH_BIT),
    S8(S8_UINT, STENCIL_BIT),
    D16_S8(D16_UNORM_S8_UINT, DEPTH_BIT | STENCIL_BIT),
    D32F_S8(D32_SFLOAT_S8_UINT, DEPTH_BIT | STENCIL_BIT),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
crate enum Dimension { One, Two, Three, Four }

crate type ChannelCount = Dimension;

impl Format {
    crate fn is_depth_stencil(self) -> bool {
        use vk::ImageAspectFlags as Flags;
        self.aspects().intersects(Flags::DEPTH_BIT | Flags::STENCIL_BIT)
    }
}

impl From<Dimension> for u32 {
    fn from(dim: Dimension) -> Self {
        match dim {
            Dimension::One => 1,
            Dimension::Two => 2,
            Dimension::Three => 3,
            Dimension::Four => 4,
        }
    }
}

impl From<Dimension> for usize {
    fn from(dim: Dimension) -> Self {
        u32::from(dim) as _
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn smoke_test(_: testing::TestVars) {
        use Dimension::*;
        use Format::*;

        let fmt = RGBA16F;
        assert!(!fmt.is_depth_stencil());
        assert_eq!(vk::Format::from(fmt), vk::Format::R16G16B16A16_SFLOAT);

        let fmt = BGRA8_SRGB;
        assert!(!fmt.is_depth_stencil());
        assert_eq!(vk::Format::from(fmt), vk::Format::B8G8R8A8_SRGB);

        let fmt = D16;
        assert!(fmt.is_depth_stencil());
        assert_eq!(vk::Format::from(fmt), vk::Format::D16_UNORM);

        let fmt = S8;
        assert!(fmt.is_depth_stencil());
        assert_eq!(vk::Format::from(fmt), vk::Format::S8_UINT);

        let fmt = D32F_S8;
        assert!(fmt.is_depth_stencil());
        assert_eq!(vk::Format::from(fmt), vk::Format::D32_SFLOAT_S8_UINT);
    }

    unit::declare_tests![smoke_test];
}

unit::collect_tests![tests];
