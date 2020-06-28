use std::ptr;
use std::sync::Arc;

use bitflags::*;
use derivative::*;
use more_asserts::assert_le;

use crate::*;

bitflags! {
    #[derive(Default)]
    pub(crate) struct ImageFlags: u32 {
        /// Image will not be sampled by shaders.
        const NO_SAMPLE = bit!(0);
        /// Image may be used as a shader storage image.
        // TODO: Will this *really* ever get used?
        const STORAGE = bit!(1);
        /// Image may be used as a color attachment.
        const COLOR_ATTACHMENT = bit!(2);
        /// Image may be used as a depth/stencil attachment.
        const DEPTH_STENCIL_ATTACHMENT = bit!(3);
        /// Image may be used as an input attachment.
        const INPUT_ATTACHMENT = bit!(4);
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
crate enum ImageType {
    /// One-dimensional image or image array.
    Dim1,
    /// Two-dimensional image or image array other than a cube map.
    Dim2,
    /// Three-dimensional image.
    Dim3,
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
    alloc: DeviceAlloc,
}

#[derive(Debug)]
pub struct ImageView {
    image: Arc<Image>,
    ty: vk::ImageViewType,
    format: Format,
    components: vk::ComponentMapping,
    subresources: vk::ImageSubresourceRange,
    inner: vk::ImageView,
}

impl Drop for Image {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            dt.destroy_image(self.inner, ptr::null());
        }
    }
}

impl Image {
    crate unsafe fn new(
        heap: &DeviceHeap,
        flags: ImageFlags,
        ty: ImageType,
        format: Format,
        samples: SampleCount,
        extent: Extent3D,
        mip_levels: u32,
        layers: u32,
    ) -> Self {
        let device: Arc<Device> = Arc::clone(heap.device());
        let dt = &*device.table;

        validate_image_creation(&device, flags, ty, format, samples,
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

        // TODO: constructor should eventually be generic over allocator
        let alloc = heap.alloc_image_memory(image, MemoryMapping::Unmapped);
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
            alloc,
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

    crate fn all_layers(&self, mip_level: u32) -> vk::ImageSubresourceLayers {
        vk::ImageSubresourceLayers {
            aspect_mask: self.format.aspects(),
            mip_level,
            base_array_layer: 0,
            layer_count: self.layers,
        }
    }

    crate fn create_full_view(self: &Arc<Self>) -> Arc<ImageView> {
        use ImageType::*;
        use vk::ImageViewType as T;
        let ty = match self.ty {
            Dim1 if self.layers == 0 => T::_1D,
            Dim1 => T::_1D_ARRAY,
            Dim2 if self.layers == 0 => T::_2D,
            Dim2 => T::_2D_ARRAY,
            Dim3 => T::_3D,
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

    crate fn inner(&self) -> vk::Image {
        self.inner
    }

    crate fn flags(&self) -> ImageFlags {
        self.flags
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

    fn size_of(
        &self,
        aspects: vk::ImageAspectFlags,
        levels: std::ops::Range<u32>,
        layers: u32,
    ) -> vk::DeviceSize {
        assert_eq!(aspects, self.format.aspects());
        assert_eq!(self.samples, SampleCount::One);
        assert_le!(levels.end, self.mip_levels);
        let lvl_size = |lvl| self.extent().mip_level(lvl).texel_count();
        let texels: vk::DeviceSize = levels.map(lvl_size).sum();
        texels * self.format.size() as vk::DeviceSize
            * layers as vk::DeviceSize
    }

    crate fn subresource_size(&self, subresource: vk::ImageSubresource) ->
        vk::DeviceSize
    {
        let aspects = subresource.aspect_mask;
        let level = subresource.mip_level;
        self.size_of(aspects, level..level + 1, 1)
    }

    crate fn subresource_range_size(
        &self,
        subresource: vk::ImageSubresourceRange,
    ) -> vk::DeviceSize {
        let aspects = subresource.aspect_mask;
        let start = subresource.base_mip_level;
        let len = subresource.level_count;
        let layers = subresource.layer_count;
        self.size_of(aspects, start..start+len, layers)
    }

    crate fn subresource_layers_size(
        &self,
        subresource: vk::ImageSubresourceLayers,
    ) -> vk::DeviceSize {
        let aspects = subresource.aspect_mask;
        let level = subresource.mip_level;
        let layers = subresource.layer_count;
        self.size_of(aspects, level..level + 1, layers)
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

    crate fn inner(&self) -> vk::ImageView {
        self.inner
    }

    crate fn image(&self) -> &Arc<Image> {
        &self.image
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
            Self::Dim1 => &[T::_1D, T::_1D_ARRAY],
            Self::Dim2 => &[T::_2D, T::_2D_ARRAY],
            // 2D_ARRAY_COMPATIBLE_BIT not supported
            Self::Dim3 => &[T::_3D],
            Self::Cube => &[T::_2D, T::_2D_ARRAY, T::CUBE, T::CUBE_ARRAY],
        };
        compat.contains(&view)
    }
}

impl From<ImageType> for vk::ImageType {
    fn from(ty: ImageType) -> Self {
        use ImageType::*;
        match ty {
            Dim1 => vk::ImageType::_1D,
            Dim2 | Cube => vk::ImageType::_2D,
            Dim3 => vk::ImageType::_3D,
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
    assert!(extent.as_array().iter().all(|&x| x > 0));
    assert!(mip_levels > 0);
    assert!(mip_levels <= extent.mip_levels());
    assert!(layers > 0);

    let limits = device.limits();
    let max_dim = match ty {
        ImageType::Dim1 => limits.max_image_dimension_1d,
        ImageType::Dim2 => limits.max_image_dimension_2d,
        ImageType::Dim3 => limits.max_image_dimension_3d,
        ImageType::Cube => limits.max_image_dimension_cube,
    };
    assert!(extent.as_array().iter().all(|&x| x <= max_dim));
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
        assert!(base.checked_add(len).unwrap() <= max);
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

    unsafe fn creation(vars: testing::TestVars) {
        use ImageFlags as Flags;

        let device = Arc::clone(vars.device());
        let state = SystemState::new(Arc::clone(&device));
        let heap = &state.heap;

        // Create some render targets
        let extent = Extent3D::new(320, 200, 1);
        let hdr = Arc::new(Image::new(
            &heap,
            Flags::NO_SAMPLE | Flags::COLOR_ATTACHMENT,
            ImageType::Dim2,
            Format::RGBA16F,
            SampleCount::Four,
            extent,
            1,
            1,
        ));
        let _hdr_view = hdr.create_full_view();
        let depth = Arc::new(Image::new(
            &heap,
            Flags::NO_SAMPLE | Flags::DEPTH_STENCIL_ATTACHMENT,
            ImageType::Dim2,
            Format::D32F_S8,
            SampleCount::Four,
            extent,
            1,
            1,
        ));
        let _depth_view = depth.create_full_view();

        // HDR cube texture
        let env = Arc::new(Image::new(
            &heap,
            Default::default(),
            ImageType::Cube,
            Format::RGBA16F,
            SampleCount::One,
            Extent3D::new(256, 256, 1),
            1,
            6,
        ));
        let _env_view = env.create_full_view();
    }

    unsafe fn subresource_size(vars: testing::TestVars) {
        use ImageFlags as Flags;

        let device = Arc::clone(vars.device());
        let state = SystemState::new(Arc::clone(&device));
        let heap = &state.heap;

        let extent = Extent3D::new(128, 128, 1);
        let img = Arc::new(Image::new(
            &heap,
            Flags::NO_SAMPLE | Flags::COLOR_ATTACHMENT,
            ImageType::Dim2,
            Format::RGBA8,
            SampleCount::One,
            extent,
            extent.mip_levels(),
            6,
        ));

        let tx_size = img.format().size() as vk::DeviceSize;
        assert_eq!(img.subresource_size(vk::ImageSubresource {
            aspect_mask: vk::ImageAspectFlags::COLOR_BIT,
            mip_level: 0,
            array_layer: 1,
        }), 128 * 128 * tx_size);
        assert_eq!(img.subresource_size(vk::ImageSubresource {
            aspect_mask: vk::ImageAspectFlags::COLOR_BIT,
            mip_level: 3,
            array_layer: 1,
        }), 16 * 16 * tx_size);
        assert_eq!(img.subresource_layers_size(vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR_BIT,
            mip_level: 0,
            base_array_layer: 1,
            layer_count: 3,
        }), 128 * 128 * tx_size * 3);
        let tx_count = 128 * 128 + 64 * 64 + 32 * 32 + 16 * 16 + 8 * 8 + 4 * 4
            + 2 * 2 + 1 * 1;
        assert_eq!(img.subresource_range_size(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR_BIT,
            base_mip_level: 0,
            level_count: extent.mip_levels(),
            base_array_layer: 0,
            layer_count: 6,
        }), tx_count * tx_size * 6);
    }

    unit::declare_tests![creation, subresource_size];
}

unit::collect_tests![tests];
