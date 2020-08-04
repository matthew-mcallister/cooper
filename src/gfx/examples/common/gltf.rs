use std::sync::Arc;

use anyhow::{self as any, anyhow, Context, Error};
use base::{PartialEnumMap, partial_map_opt};
use cooper_gfx::*;
use fehler::{throw, throws};
use gltf::{accessor, mesh};
use math::vector::{Vector3, vec};

#[derive(Debug)]
crate struct GltfBundle {
    crate path: String,
    crate document: gltf::Document,
    crate buffers: Vec<gltf::buffer::Data>,
    crate images: Vec<ImageData>,
}

#[derive(Debug)]
crate struct ImageData {
    crate pixels: Arc<Vec<u8>>,
    crate format: gltf::image::Format,
    crate width: u32,
    crate height: u32,
}

crate type BBox = [Vector3<f32>; 2];

#[derive(Debug)]
crate struct Mesh {
    crate bbox: BBox,
    crate render_mesh: Arc<RenderMesh>,
    crate images: MaterialImageBindings,
}

#[throws]
fn load_meshes(
    rl: &mut RenderLoop,
    bundle: &GltfBundle,
) -> Vec<Mesh> {
    bundle.document.meshes().flat_map(|mesh| mesh.primitives())
        .map(move |prim| Mesh::from_primitive(rl, bundle, &prim))
        .collect::<Result<_, _>>()?
}

impl GltfBundle {
    crate fn import(path: impl Into<String>) -> gltf::Result<Self> {
        let path = path.into();
        let (document, buffers, images) = gltf::import(&path)?;

        let images = images.into_iter()
            .map(ImageData::from)
            .collect();

        Ok(Self { path, document, buffers, images })
    }

    #[throws]
    #[inline]
    fn accessor_view_data(&self, accessor: &gltf::Accessor<'_>) -> &[u8] {
        let view = accessor.view().ok_or(anyhow!("sparse accessor"))?;
        &self.buffers[view.buffer().index()][view.offset()..]
    }

    #[throws]
    fn read_attr_accessor(&self, accessor: &gltf::Accessor<'_>) ->
        (Format, &[u8])
    {
        let format = map_format(
            accessor.data_type(),
            accessor.dimensions(),
            accessor.normalized(),
        )?;
        let data = self.accessor_view_data(accessor)?;
        let len = accessor.count() * format.size();
        (format, &data[..len])
    }

    #[throws]
    fn read_index_accessor(&self, accessor: &gltf::Accessor<'_>) ->
        (IndexType, &[u8])
    {
        let ty = map_index_type(accessor.data_type())?;
        let data = self.accessor_view_data(accessor)?;
        let len = accessor.count() * ty.size();
        (ty, &data[..len])
    }

    crate fn load_meshes(&self, rloop: &mut RenderLoop) ->
        any::Result<Vec<Mesh>>
    {
        load_meshes(rloop, self)
    }
}

impl Mesh {
    crate fn from_primitive(
        rl: &mut RenderLoop,
        bundle: &GltfBundle,
        prim: &gltf::Primitive<'_>,
    ) -> any::Result<Self> {
        // TODO: would like an easier way to attach this info
        // TODO: why is there no way to get the parent mesh index?!
        from_primitive(rl, bundle, prim).context(format!(
            "path: `{}`, primitive: {}", bundle.path, prim.index()))
    }
}

#[throws]
fn from_primitive(
    rloop: &mut RenderLoop,
    bundle: &GltfBundle,
    prim: &gltf::Primitive<'_>,
) -> Mesh {
    Mesh {
        bbox: get_bbox(prim),
        render_mesh: load_mesh(rloop, bundle, prim)?,
        images: load_material_images(rloop, bundle, prim.material())?,
    }
}

fn get_bbox(prim: &gltf::Primitive<'_>) -> BBox {
    let bbox = prim.bounding_box();
    [vec(bbox.min), vec(bbox.max)]
}

macro_rules! try_as {
    ($err:ty, $($body:tt)*) => {
        (try { $($body)* }: Result<_, $err>)
    }
}

// I swear this library needs a proc macro to make mixing contexts with
// control flow at all reasonable.
macro_rules! with_context {
    (
        where context = $context:expr;
        $($body:tt)*
    ) => {
        try_as!(anyhow::Error, { $($body)* }).with_context($context)
    }
}

#[throws]
fn load_mesh(
    rloop: &mut RenderLoop,
    bundle: &GltfBundle,
    prim: &gltf::Primitive<'_>,
) -> Arc<RenderMesh> {
    use VertexAttr::*;

    tassert!(prim.mode() == mesh::Mode::Triangles,
        "unsupported primitive topology: {:?}", prim.mode());
    tassert!(!prim.attributes().is_empty(), "no attribute data");

    let vertex_count = prim.attributes().next().unwrap().1.count();
    let mut attrs: PartialEnumMap<_, _> = Default::default();
    for (sem, accessor) in prim.attributes() { with_context!(
        where context = || format!("attribute {:?}", sem);
        tassert!(
            accessor.count() == vertex_count,
            "accessor size: got: {}, expected: {}",
            accessor.count(), vertex_count,
        );
        let attr = map_semantic(&sem)?;
        let (format, data) = bundle.read_attr_accessor(&accessor)?;
        attrs.insert(attr, (format, data));
    )?; }

    for &attr in &[Position, Normal, Texcoord0] {
        tassert!(attrs.contains_key(attr), "missing attribute: {:?}", attr);
    }

    let index = tryopt!(bundle.read_index_accessor(&prim.indices()?))
        .transpose()?;

    build_mesh(rloop, attrs, index)
}

fn build_mesh(
    rloop: &mut RenderLoop,
    attrs: PartialEnumMap<VertexAttr, (Format, &[u8])>,
    index: Option<(IndexType, &[u8])>,
) -> Arc<RenderMesh> {
    let mut builder = RenderMeshBuilder::from_loop(rloop);
    builder.lifetime(Lifetime::Static);

    for (attr, &(fmt, data)) in attrs.iter() {
        builder.attr(attr, fmt, data);
    }

    if let Some((ty, buf)) = index {
        builder.index(ty, buf);
    }

    unsafe { Arc::new(builder.build()) }
}

#[throws]
fn map_semantic(sem: &gltf::Semantic) -> VertexAttr {
    use gltf::Semantic;
    match sem {
        Semantic::Positions => VertexAttr::Position,
        Semantic::Normals => VertexAttr::Normal,
        Semantic::Tangents => VertexAttr::Tangent,
        Semantic::Colors(0) => VertexAttr::Color,
        Semantic::TexCoords(0) => VertexAttr::Texcoord0,
        Semantic::TexCoords(1) => VertexAttr::Texcoord1,
        Semantic::Joints(0) => VertexAttr::Joints,
        Semantic::Weights(0) => VertexAttr::Weights,
        _ => throw!(anyhow!("unsupported semantic")),
    }
}

// Used for reading vertex data.
#[throws]
fn map_format(
    ty: accessor::DataType,
    shape: accessor::Dimensions,
    normalized: bool,
) -> Format {
    use accessor::{DataType as Type, Dimensions as Dim};
    match (ty, shape, normalized) {
        (Type::U8, Dim::Scalar, true) => Format::R8,
        (Type::U8, Dim::Vec2, true) => Format::RG8,
        (Type::U8, Dim::Vec3, true) => Format::RGB8,
        (Type::U8, Dim::Vec4, true) => Format::RGBA8,
        (Type::U8, Dim::Vec4, false) => Format::RGBA8U,
        (Type::U16, Dim::Vec4, false) => Format::RGBA16U,
        (Type::F32, Dim::Scalar, _) => Format::R32F,
        (Type::F32, Dim::Vec2, _) => Format::RG32F,
        (Type::F32, Dim::Vec3, _) => Format::RGB32F,
        (Type::F32, Dim::Vec4, _) => Format::RGBA32F,
        _ => throw!(anyhow!(
            "unsupported format: {:?}",
            (ty, shape, normalized),
        )),
    }
}

#[throws]
fn map_index_type(ty: accessor::DataType) -> IndexType {
    use accessor::DataType;
    match ty {
        DataType::U16 => IndexType::U16,
        DataType::U32 => IndexType::U32,
        _ => throw!(anyhow!("bad index type: {:?}", ty)),
    }
}

// FIXME: This is going to load a ton of duplicate textures
#[throws]
fn load_material_images(
    rloop: &mut RenderLoop,
    bundle: &GltfBundle,
    material: gltf::Material<'_>,
) -> MaterialImageBindings {
    tassert!(material.alpha_mode() == gltf::material::AlphaMode::Opaque,
        "transparency not supported");

    let normal = if let Some(binding) = material.normal_texture() {
        tassert!(binding.tex_coord() == 0, "texcoord != 0");
        tassert!(binding.scale() == 1.0, "normal scale != 1");
        Some(load_texture(rloop, bundle, binding.texture())?)
    } else { None };

    let pbr = material.pbr_metallic_roughness();

    macro_rules! try_load_texture { ($texture:expr) => {
        if let Some(binding) = $texture {
            tassert!(binding.tex_coord() == 0, "texcoord != 0");
            Some(load_texture(rloop, bundle, binding.texture())?)
        } else { None }
    } }

    let albedo = try_load_texture!(pbr.base_color_texture());
    let metal_rough = try_load_texture!(pbr.metallic_roughness_texture());

    // NB: This is fixed in the upcoming version of fehler
    #[allow(unused_parens)]
    (partial_map_opt! {
        MaterialImage::Albedo => albedo,
        MaterialImage::Normal => normal,
        MaterialImage::MetallicRoughness => metal_rough,
    })
}

#[throws]
fn load_texture(
    rloop: &mut RenderLoop,
    bundle: &GltfBundle,
    tex: gltf::texture::Texture<'_>,
) -> ImageBindingDesc {
    let data = &bundle.images[tex.source().index() as usize];

    let image = rloop.define_image(
        Default::default(),
        ImageType::Dim2,
        format(data.format)?,
        (data.width, data.height).into(),
        1,
        1,
        Some(source_string(bundle, tex.source().source())),
    );
    rloop.upload_image(&image, Arc::clone(&data.pixels), 0);

    let sampler_state = load_sampler(tex.sampler());
    ImageBindingDesc {
        subresources: image.all_subresources(),
        image,
        sampler_state,
    }
}

fn source_string(bundle: &GltfBundle, src: gltf::image::Source<'_>) -> String {
    match src {
        gltf::image::Source::View { view, .. } =>
            format!("{}:{}", bundle.path, view.index()),
        gltf::image::Source::Uri { uri, .. } => uri.into(),
    }
}

#[throws]
fn format(format: gltf::image::Format) -> Format {
    use gltf::image::Format as GltfFormat;
    match format {
        GltfFormat::R8 => Format::R8,
        GltfFormat::R8G8 => Format::RG8,
        GltfFormat::R8G8B8 => Format::RGB8,
        GltfFormat::R8G8B8A8 => Format::RGBA8,
        GltfFormat::B8G8R8A8 => Format::BGRA8,
        // Can't find enough data on the web to know if other formats
        // are commonly supported in shaders.
        _ => throw!(anyhow!("incompatible image format")),
    }
}

fn load_sampler(sampler: gltf::texture::Sampler<'_>) -> SamplerDesc {
    let mag_filter = tryopt!(mag_filter(sampler.mag_filter()?))
        .unwrap_or(Filter::Linear);
    let (min_filter, mipmap_mode) = tryopt!(min_filter(sampler.min_filter()?))
        .unwrap_or((Filter::Linear, SamplerMipmapMode::Linear));
    let address_mode_u = wrapping_mode(sampler.wrap_s());
    let address_mode_v = wrapping_mode(sampler.wrap_t());
    SamplerDesc {
        mag_filter,
        min_filter,
        mipmap_mode,
        address_mode_u,
        address_mode_v,
        anisotropy_level: AnisotropyLevel::Sixteen,
        ..Default::default()
    }
}

fn mag_filter(filter: gltf::texture::MagFilter) -> Filter {
    use gltf::texture::MagFilter;
    match filter {
        MagFilter::Nearest => Filter::Nearest,
        MagFilter::Linear => Filter::Linear,
    }
}

fn min_filter(filter: gltf::texture::MinFilter) -> (Filter, SamplerMipmapMode)
{
    use gltf::texture::MinFilter;
    match filter {
        MinFilter::Nearest | MinFilter::NearestMipmapNearest =>
            (Filter::Nearest, SamplerMipmapMode::Nearest),
        MinFilter::LinearMipmapNearest =>
            (Filter::Linear, SamplerMipmapMode::Nearest),
        MinFilter::NearestMipmapLinear =>
            (Filter::Nearest, SamplerMipmapMode::Linear),
        MinFilter::Linear | MinFilter::LinearMipmapLinear =>
            (Filter::Linear, SamplerMipmapMode::Linear),
    }
}

fn wrapping_mode(wrapping_mode: gltf::texture::WrappingMode) ->
    SamplerAddressMode
{
    use gltf::texture::WrappingMode;
    match wrapping_mode {
        WrappingMode::ClampToEdge => SamplerAddressMode::ClampToEdge,
        WrappingMode::MirroredRepeat => SamplerAddressMode::MirroredRepeat,
        WrappingMode::Repeat => SamplerAddressMode::Repeat,
    }
}

impl From<gltf::image::Data> for ImageData {
    fn from(data: gltf::image::Data) -> Self {
        Self {
            pixels: Arc::new(data.pixels),
            format: data.format,
            width: data.width,
            height: data.height,
        }
    }
}
