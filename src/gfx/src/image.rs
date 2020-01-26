use std::mem::ManuallyDrop;
use std::ptr;
use std::sync::Arc;

use bitflags::*;
use derivative::*;

// TODO: This issues an unused import warning which is a bug
use crate::*;

bitflags! {
    #[derive(Default)]
    pub(crate) struct ImageFlags: u32 {
        /// Image will not be sampled by shaders.
        const NO_SAMPLE = bit!(0);
        /// Image may be used as a shader storage image.
        const STORAGE = bit!(1);
        /// Image may be used as a color attachment.
        const COLOR_ATTACHMENT = bit!(2);
        /// Image may be used as a depth/stencil attachment.
        const DEPTH_STENCIL_ATTACHMENT = bit!(3);
        /// Image may be used as an input attachment.
        const INPUT_ATTACHMENT = bit!(4);
    }
}

// TODO: These variant names suck
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
crate enum ImageType {
    /// One-dimensional image or image array.
    OneDim,
    /// Two-dimensional image or image array other than a cube map.
    TwoDim,
    /// Three-dimensional image.
    ThreeDim,
    /// A cube or cube array, which is a type of 2D array.
    Cube,
}

#[derive(Clone, Copy, Debug, Derivative, Eq, Hash, PartialEq)]
#[derivative(Default)]
crate enum SampleCount {
    #[derivative(Default)]
    One,
    Two,
    Four,
    Eight,
    Sixteen,
    ThirtyTwo,
    SixtyFour,
}

#[derive(Debug)]
crate struct Image {
    device: Arc<Device>,
    flags: ImageFlags,
    ty: ImageType,
    format: Format,
    samples: SampleCount,
    extent: Extent3D,
    mip_levels: u32,
    layers: u32,
    inner: vk::Image,
    // TODO: Memory allocations really need to follow RAII
    alloc: ManuallyDrop<DeviceRange>,
    state: Arc<SystemState>,
}

#[derive(Debug)]
crate struct ImageView {
    image: Arc<Image>,
    ty: vk::ImageViewType,
    format: Format,
    components: vk::ComponentMapping,
    subresources: vk::ImageSubresourceRange,
    inner: vk::ImageView,
}

impl Drop for Image {
    fn drop(&mut self) {
        let dt = &*self.state.device.table;
        unsafe {
            dt.destroy_image(self.inner, ptr::null());
            let mut heap = self.state.heap.lock();
            heap.free(ManuallyDrop::take(&mut self.alloc));
        }
    }
}

impl Image {
    crate unsafe fn new(
        state: Arc<SystemState>,
        flags: ImageFlags,
        ty: ImageType,
        format: Format,
        samples: SampleCount,
        extent: Extent3D,
        mip_levels: u32,
        layers: u32,
    ) -> Self {
        let device: Arc<Device> = Arc::clone(state.device());
        let dt = &*device.table;

        validate_image_creation(&state.device, flags, ty, format, samples,
            extent, mip_levels, layers);

        let create_info = vk::ImageCreateInfo {
            flags: ty.flags(),
            image_type: ty.into(),
            format: format.into(),
            extent: extent.into(),
            mip_levels,
            array_layers: layers,
            samples: samples.into(),
            tiling: vk::ImageTiling::OPTIMAL,
            usage: flags.usage(),
            ..Default::default()
        };
        let mut image = vk::null();
        dt.create_image(&create_info, ptr::null(), &mut image)
            .check().unwrap();

        let alloc = {
            let mut heap = state.heap.lock();
            heap.alloc_image_memory(image, MemoryMapping::Unmapped)
        };

        Image {
            device,
            flags,
            ty,
            format,
            samples,
            extent,
            mip_levels,
            layers,
            inner: image,
            alloc: ManuallyDrop::new(alloc),
            state,
        }
    }

    crate fn all_subresources(&self) -> vk::ImageSubresourceRange {
        vk::ImageSubresourceRange {
            aspect_mask: self.format.aspects(),
            base_mip_level: 0,
            level_count: self.mip_levels,
            base_array_layer: 0,
            layer_count: self.layers,
        }
    }

    crate fn create_default_view(self: &Arc<Self>) -> Arc<ImageView> {
        use ImageType::*;
        use vk::ImageViewType as T;
        let ty = match self.ty {
            OneDim if self.layers == 0 => T::_1D,
            OneDim => T::_1D_ARRAY,
            TwoDim if self.layers == 0 => T::_2D,
            TwoDim => T::_2D_ARRAY,
            ThreeDim => T::_3D,
            Cube if self.layers == 0 => T::CUBE,
            Cube => T::CUBE_ARRAY,
        };
        // This ought to be safe if it isn't
        unsafe {
            Arc::new(ImageView::new(
                Arc::clone(self),
                ty,
                self.format,
                Default::default(),
                self.all_subresources(),
            ))
        }
    }

    crate fn format(&self) -> Format {
        self.format
    }

    crate fn samples(&self) -> SampleCount {
        self.samples
    }

    crate fn extent(&self) -> Extent3D {
        self.extent
    }

    crate fn layers(&self) -> u32 {
        self.layers
    }
}

impl Drop for ImageView {
    fn drop(&mut self) {
        let dt = &*self.image.device.table;
        unsafe {
            dt.destroy_image_view(self.inner, ptr::null());
        }
    }
}

impl ImageView {
    crate unsafe fn new(
        image: Arc<Image>,
        ty: vk::ImageViewType,
        format: Format,
        components: vk::ComponentMapping,
        subresources: vk::ImageSubresourceRange,
    ) -> Self {
        let dt = &*image.device.table;

        validate_image_view_creation(&image, ty, format, components,
            subresources);

        let create_info = vk::ImageViewCreateInfo {
            image: image.inner,
            view_type: ty,
            format: format.into(),
            components,
            subresource_range: subresources,
            ..Default::default()
        };
        let mut view = vk::null();
        dt.create_image_view(&create_info, ptr::null(), &mut view)
            .check().unwrap();

        ImageView {
            image,
            ty,
            format,
            components,
            subresources,
            inner: view,
        }
    }

    crate fn format(&self) -> Format {
        self.format
    }

    crate fn samples(&self) -> SampleCount {
        self.image.samples
    }

    crate fn extent(&self) -> Extent3D {
        self.image.extent
    }

    crate fn subresources(&self) -> vk::ImageSubresourceRange {
        self.subresources
    }

    crate fn layers(&self) -> u32 {
        self.subresources.layer_count
    }

    crate fn mip_levels(&self) -> u32 {
        self.subresources.level_count
    }
}

impl ImageFlags {
    crate fn is_render_target(self) -> bool {
        self.intersects(Self::COLOR_ATTACHMENT
            | Self::DEPTH_STENCIL_ATTACHMENT)
    }

    crate fn is_attachment(self) -> bool {
        self.intersects(Self::COLOR_ATTACHMENT
            | Self::DEPTH_STENCIL_ATTACHMENT
            | Self::INPUT_ATTACHMENT)
    }

    crate fn usage(self) -> vk::ImageUsageFlags {
        use vk::ImageUsageFlags as F;

        let pairs = [
            (Self::STORAGE, F::STORAGE_BIT),
            (Self::COLOR_ATTACHMENT, F::COLOR_ATTACHMENT_BIT),
            (Self::DEPTH_STENCIL_ATTACHMENT, F::DEPTH_STENCIL_ATTACHMENT_BIT),
            (Self::INPUT_ATTACHMENT, F::INPUT_ATTACHMENT_BIT),
        ];
        let mut usage = pairs.iter().cloned()
            .filter_map(|(fl, vkfl)| self.contains(fl).then_some(vkfl))
            .fold(Default::default(), |acc, flag| acc | flag);

        if !self.contains(Self::NO_SAMPLE) {
            usage |= F::SAMPLED_BIT;
        }
        if !self.is_render_target() {
            usage |= F::TRANSFER_DST_BIT;
        }

        usage
    }
}

impl ImageType {
    fn flags(self) -> vk::ImageCreateFlags {
        match self {
            Self::Cube => vk::ImageCreateFlags::CUBE_COMPATIBLE_BIT,
            _ => Default::default(),
        }
    }

    fn compat_view(self, view: vk::ImageViewType) -> bool {
        use vk::ImageViewType as T;
        let compat: &[vk::ImageViewType] = match self {
            Self::OneDim => &[T::_1D, T::_1D_ARRAY],
            Self::TwoDim => &[T::_2D, T::_2D_ARRAY],
            // 2D_ARRAY_COMPATIBLE_BIT not supported
            Self::ThreeDim => &[T::_3D],
            Self::Cube => &[T::_2D, T::_2D_ARRAY, T::CUBE, T::CUBE_ARRAY],
        };
        compat.contains(&view)
    }
}

impl From<ImageType> for vk::ImageType {
    fn from(ty: ImageType) -> Self {
        use ImageType::*;
        match ty {
            OneDim => vk::ImageType::_1D,
            TwoDim | Cube => vk::ImageType::_2D,
            ThreeDim => vk::ImageType::_3D,
        }
    }
}

// Partial validation
fn validate_image_creation(
    device: &Device,
    flags: ImageFlags,
    ty: ImageType,
    format: Format,
    _samples: SampleCount,
    extent: Extent3D,
    mip_levels: u32,
    layers: u32,
) {
    assert!(extent.width > 0);
    assert!(extent.height > 0);
    assert!(extent.depth > 0);
    assert!(mip_levels > 0);
    assert!(mip_levels <= num_mip_levels(extent));
    assert!(layers > 0);

    let limits = device.limits();
    let max_dim = match ty {
        ImageType::OneDim => limits.max_image_dimension_1d,
        ImageType::TwoDim => limits.max_image_dimension_2d,
        ImageType::ThreeDim => limits.max_image_dimension_3d,
        ImageType::Cube => limits.max_image_dimension_cube,
    };
    assert!((extent.width <= max_dim) & (extent.height <= max_dim) &
            (extent.depth <= max_dim));
    assert!(layers <= limits.max_image_array_layers);

    if ty == ImageType::Cube {
        assert_eq!(extent.width, extent.height);
        assert!(layers >= 6);
    }

    let dim: vk::ImageType = ty.into();
    if dim == vk::ImageType::_1D {
        assert_eq!((extent.height, extent.depth), (1, 1));
    } else if dim == vk::ImageType::_2D {
        assert_eq!(extent.depth, 1);
    }

    if flags.is_attachment() {
        assert!(extent.width <= limits.max_framebuffer_width);
        assert!(extent.height <= limits.max_framebuffer_height);
        assert!(layers <= limits.max_framebuffer_layers);
    }

    if flags.contains(ImageFlags::DEPTH_STENCIL_ATTACHMENT) {
        assert!(format.is_depth_stencil());
    }
}

crate fn num_mip_levels(Extent3D { width, height, depth }: Extent3D) -> u32 {
    use std::cmp::max;
    fn log2(n: u32) -> u32 {
        assert!(n > 0);
        31 - n.leading_zeros()
    }
    max(max(log2(width), log2(height)), log2(depth)) + 1
}

// Partial validation
fn validate_image_view_creation(
    image: &Image,
    ty: vk::ImageViewType,
    format: Format,
    _components: vk::ComponentMapping,
    range: vk::ImageSubresourceRange,
) {
    assert!(image.ty.compat_view(ty), "{:?}, {:?}", image.ty, ty);

    if ty == vk::ImageViewType::CUBE {
        assert_eq!(range.layer_count, 6);
    } else if ty == vk::ImageViewType::CUBE_ARRAY {
        assert_eq!(range.layer_count % 6, 0);
    }

    // MUTABLE_FORMAT_BIT not yet supported
    assert_eq!(format, image.format);

    // TODO: Check format is compatible

    assert!(format.aspects().contains(range.aspect_mask));

    fn range_check(base: u32, len: u32, max: u32) {
        // The first two statements guard against overflow
        assert!(base < max);
        assert!(len > 0);
        assert!(base + len <= max);
    }
    range_check(range.base_mip_level, range.level_count, image.mip_levels);
    range_check(range.base_array_layer, range.layer_count, image.layers);
}

impl From<SampleCount> for vk::SampleCountFlags {
    fn from(samples: SampleCount) -> Self {
        use vk::SampleCountFlags as Flags;
        use SampleCount::*;
        match samples {
            One => Flags::_1_BIT,
            Two => Flags::_2_BIT,
            Four => Flags::_4_BIT,
            Eight => Flags::_8_BIT,
            Sixteen => Flags::_16_BIT,
            ThirtyTwo => Flags::_32_BIT,
            SixtyFour => Flags::_64_BIT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        use ImageFlags as Flags;

        let device = Arc::clone(vars.device());
        let state = Arc::new(SystemState::new(Arc::clone(&device)));

        // Create some render targets
        let extent = Extent3D::new(320, 200, 1);
        let hdr = Arc::new(Image::new(
            Arc::clone(&state),
            Flags::NO_SAMPLE | Flags::COLOR_ATTACHMENT,
            ImageType::TwoDim,
            Format::RGB16F,
            SampleCount::Four,
            extent,
            1,
            1,
        ));
        let _hdr_view = hdr.create_default_view();
        let depth = Arc::new(Image::new(
            Arc::clone(&state),
            Flags::NO_SAMPLE | Flags::DEPTH_STENCIL_ATTACHMENT,
            ImageType::TwoDim,
            Format::D32F_S8,
            SampleCount::Four,
            extent,
            1,
            1,
        ));
        let _depth_view = depth.create_default_view();

        // HDR cube texture
        let env = Arc::new(Image::new(
            Arc::clone(&state),
            Default::default(),
            ImageType::Cube,
            Format::RGB16F,
            SampleCount::One,
            Extent3D::new(256, 256, 1),
            1,
            6,
        ));
        let _env_view = env.create_default_view();
    }

    unit::declare_tests![smoke_test];
}

unit::collect_tests![tests];
