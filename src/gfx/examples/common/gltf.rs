use std::sync::Arc;

use anyhow::{self as any, anyhow, Context, Error};
use base::partial_map;
use cooper_gfx::*;
use fehler::{throw, throws};
use gltf::{accessor, mesh};

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

#[derive(Debug)]
crate struct Mesh {
    crate bbox: std::ops::Range<[f32; 3]>,
    crate render_mesh: Arc<RenderMesh>,
    crate images: MaterialImageBindings,
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

    crate fn get_buffer_view(&self, view: &gltf::buffer::View<'_>) -> &[u8] {
        let data = &self.buffers[view.buffer().index()];
        &data[view.offset()..view.offset() + view.length()]
    }
}

impl Mesh {
    /// Creates a mesh from the first primitive of the first mesh in a model
    /// file.
    crate fn from_gltf(rl: &mut RenderLoop, bundle: &GltfBundle) ->
        any::Result<Self>
    {
        let prim: Option<_> = try {
            bundle.document.meshes().next()?.primitives().next()?
        };
        let prim = prim.ok_or_else(|| anyhow!("no primitives: path: `{}`"))?;
        Self::from_primitive(rl, bundle, &prim)
    }

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

fn get_bbox(prim: &gltf::Primitive<'_>) -> std::ops::Range<[f32; 3]> {
    let bbox = prim.bounding_box();
    bbox.min..bbox.max
}

#[throws]
fn load_mesh(
    rl: &mut RenderLoop,
    bundle: &GltfBundle,
    prim: &gltf::Primitive<'_>,
) -> Arc<RenderMesh> {
    tassert!(prim.mode() == mesh::Mode::Triangles,
        anyhow!("unsupported primitive topology: {:?}", prim.mode()));

    let mut attrs = Vec::new();
    for (sem, accessor) in prim.attributes() {
        // TODO: try block results in bad indentation
        let res: any::Result<_> = try {
            let attr = map_semantic(&sem)?;
            let format = map_format(
                accessor.data_type(),
                accessor.dimensions(),
                accessor.normalized(),
            )?;

            let view = accessor.view()
                .ok_or_else(|| anyhow!("sparse accessor"))?;
            let data = bundle.get_buffer_view(&view);
            attrs.push((attr, format, data));
        };
        res.with_context(|| format!("attribute {:?}", sem))?;
    }
    tassert!(!prim.attributes().is_empty(), anyhow!("no attribute data"));

    let index: Result<_, any::Error> = prim.indices()
        .map(|accessor| {
            let data_ty = accessor.data_type();
            let ty = map_index_type(data_ty)?;
            let data = bundle.get_buffer_view(&accessor_view(&accessor)?);
            Ok((ty, data))
        })
        .transpose();
    let index = index?;

    let mut counts = attrs.iter()
        .map(|(_, fmt, data)| data.len() / fmt.size());
    let vertex_count = counts.clone().next().unwrap();
    tassert!(counts.all(|count| count == vertex_count),
        anyhow!("attribute counts not equal"));

    let mut builder = RenderMeshBuilder::from_loop(rl);
    builder.lifetime(Lifetime::Static);
    for &(attr, fmt, data) in attrs.iter() {
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
        (Type::U16, Dim::Vec4, true) => Format::RG16,
        (Type::F32, Dim::Scalar, _) => Format::R32F,
        (Type::F32, Dim::Vec2, _) => Format::RG32F,
        (Type::F32, Dim::Vec3, _) => Format::RGB32F,
        (Type::F32, Dim::Vec4, _) => Format::RGBA32F,
        _ => throw!(anyhow!("unsupported format")),
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

#[throws]
fn accessor_view<'a>(accessor: &'a accessor::Accessor<'a>) ->
    gltf::buffer::View<'a>
{
    accessor.view().ok_or(anyhow!("sparse accessor"))?
}

#[throws]
fn load_material_images(
    rloop: &mut RenderLoop,
    bundle: &GltfBundle,
    material: gltf::Material<'_>,
) -> MaterialImageBindings {
    tassert!(material.alpha_mode() == gltf::material::AlphaMode::Opaque,
        anyhow!("transparency not supported"));

    let binding = material.normal_texture()
        .ok_or(anyhow!("missing normal texture"))?;
    tassert!(binding.tex_coord() == 0, anyhow!("expected texcoord == 0"));
    tassert!(binding.scale() == 1.0, anyhow!("expected normal scale == 1"));
    let normal = load_texture(rloop, bundle, binding.texture())?;

    let pbr = material.pbr_metallic_roughness();

    let binding = pbr.base_color_texture()
        .ok_or(anyhow!("missing albedo texture"), )?;
    tassert!(binding.tex_coord() == 0, anyhow!("expected texcoord == 0"));
    let albedo = load_texture(rloop, bundle, binding.texture())?;

    let binding = pbr.metallic_roughness_texture()
        .ok_or(anyhow!("missing metallic_roughness texture"))?;
    tassert!(binding.tex_coord() == 0, anyhow!("expected texcoord == 0"));
    let metallic_roughness = load_texture(rloop, bundle, binding.texture())?;

    // NB: This is fixed in the upcoming version of fehler
    #[allow(unused_parens)]
    (partial_map! {
        MaterialImage::Albedo => albedo,
        MaterialImage::Normal => normal,
        MaterialImage::MetallicRoughness => metallic_roughness,
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
    );
    rloop.upload_image(
        &image,
        Arc::clone(&data.pixels),
        0,
    );

    let sampler = rloop.get_or_create_sampler(&load_sampler(tex.sampler()));

    ImageBindingDesc {
        subresources: image.all_subresources(),
        image,
        flags: Default::default(),
        sampler,
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
    let mag_filter = try_opt!(mag_filter(sampler.mag_filter()?))
        .unwrap_or(Filter::Linear);
    let (min_filter, mipmap_mode) = try_opt!(min_filter(sampler.min_filter()?))
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
