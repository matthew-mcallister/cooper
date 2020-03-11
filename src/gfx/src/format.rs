
macro_rules! impl_format {
    ($($name:ident($vk_format:ident, $size:expr, $($aspect:ident)|*),)*) => {
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

            crate fn size(self) -> usize {
                match self {$(
                    Format::$name => $size,
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
    R8(R8_UNORM, 1, COLOR_BIT),
    R16F(R16_SFLOAT, 2, COLOR_BIT),
    R32F(R32_SFLOAT, 4, COLOR_BIT),
    RG8(R8G8_UNORM, 2, COLOR_BIT),
    RG16(R16G16_UNORM, 2, COLOR_BIT),
    RG16F(R16G16_SFLOAT, 4, COLOR_BIT),
    RG32F(R32G32_SFLOAT, 8, COLOR_BIT),
    RGB8(R8G8B8_UNORM, 3, COLOR_BIT),
    RGB16F(R16G16B16_SFLOAT, 6, COLOR_BIT),
    RGB32F(R32G32B32_SFLOAT, 12, COLOR_BIT),
    RGBA8(R8G8B8A8_UNORM, 4, COLOR_BIT),
    RGBA8U(R8G8B8A8_UINT, 4, COLOR_BIT),
    RGBA16F(R16G16B16A16_SFLOAT, 8, COLOR_BIT),
    RGBA32F(R32G32B32A32_SFLOAT, 16, COLOR_BIT),
    BGRA8_SRGB(B8G8R8A8_SRGB, 4, COLOR_BIT),
    D16(D16_UNORM, 2, DEPTH_BIT),
    D32F(D32_SFLOAT, 4, DEPTH_BIT),
    S8(S8_UINT, 1, STENCIL_BIT),
    D16_S8(D16_UNORM_S8_UINT, 3, DEPTH_BIT | STENCIL_BIT),
    D32F_S8(D32_SFLOAT_S8_UINT, 5, DEPTH_BIT | STENCIL_BIT),
}

primitive_enum! {
    @[try_from: u8, u16, u32, u64, usize]
    @[try_from_error: &'static str = "not a valid dimension"]
    @[into: u8, u16, u32, u64, usize]
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    crate enum Dimension {
        One = 1,
        Two = 2,
        Three = 3,
        Four = 4,
    }
}

crate type ChannelCount = Dimension;

impl Format {
    crate fn is_depth_stencil(self) -> bool {
        use vk::ImageAspectFlags as Flags;
        self.aspects().intersects(Flags::DEPTH_BIT | Flags::STENCIL_BIT)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use super::*;

    unsafe fn smoke_test(_: testing::TestVars) {
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
