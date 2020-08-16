use std::path::{Path, PathBuf};
use std::sync::Arc;

use base::{PartialEnumMap, partial_map, partial_map_opt};
use device::*;
use derive_more::Constructor;
use fehler::{throw, throws};
use gfx::*;
use gltf::{accessor, mesh};
use log::trace;
use math::{BBox, vec};
use prelude::tryopt;

use crate::{Error, Result, load_image};
use crate::asset::*;

#[derive(Debug)]
struct Bundle {
    source: String,
    base: PathBuf,
    document: gltf::Document,
    buffers: Vec<Arc<Vec<u8>>>,
}

#[derive(Debug)]
struct Loader<'st, 'dat> {
    bundle: &'dat Bundle,
    rloop: &'st mut RenderLoop,
    assets: &'st mut AssetCache,
    images: Vec<Option<Arc<ImageDef>>>,
    // TODO: Maybe should dedup accessors in case the same mesh is
    // reused with multiple materials
}

#[derive(Constructor, Debug)]
struct SharedSlice {
    data: Arc<Vec<u8>>,
    offset: usize,
    len: usize,
}

impl Bundle {
    #[throws]
    crate fn import(path: impl Into<String>) -> Self {
        use gltf::buffer::Source;

        let source = path.into();
        let file = std::fs::File::open(&source)?;
        let gltf::Gltf { document, blob } =
            gltf::Gltf::from_reader_without_validation(file)?;

        let mut bundle = Self {
            base: Path::new(&source).parent().unwrap().into(),
            source,
            document,
            buffers: Vec::new(),
        };

        let blob = tryopt!(Arc::new(blob?));
        let bufs = bundle.document.buffers().map(|buf| Ok(match buf.source() {
            Source::Bin => Arc::clone(blob.as_ref().unwrap()),
            Source::Uri(uri) => if let Some(data) = read_data_uri(uri)? {
                Arc::new(data)
            } else {
                Arc::new(bundle.read_file(uri)?)
            },
        })).collect::<Result<Vec<_>>>()?;
        bundle.buffers = bufs;

        bundle
    }

    fn resolve_uri(&self, uri: &str) -> String {
        self.base.join(uri).into_os_string().into_string().unwrap()
    }

    #[throws]
    fn accessor_view_data(&self, accessor: &gltf::Accessor<'_>) ->
        (&Arc<Vec<u8>>, usize)
    {
        let view = accessor.view().ok_or("sparse accessor")?;
        (&self.buffers[view.buffer().index()], view.offset())
    }

    #[throws]
    fn read_attr_accessor(&self, accessor: &gltf::Accessor<'_>) ->
        (Format, SharedSlice)
    {
        let format = accessor_format(accessor)?;
        let (data, offset) = self.accessor_view_data(accessor)?;
        let len = accessor.count() * format.size();
        (format, SharedSlice::new(Arc::clone(data), offset, len))
    }

    #[throws]
    fn read_index_accessor(&self, accessor: &gltf::Accessor<'_>) ->
        (IndexType, SharedSlice)
    {
        let ty = accessor_index_type(accessor.data_type())?;
        let (data, offset) = self.accessor_view_data(accessor)?;
        let len = accessor.count() * ty.size();
        (ty, SharedSlice::new(Arc::clone(data), offset, len))
    }

    #[throws]
    fn get_view(&self, view: &gltf::buffer::View<'_>) -> &[u8] {
        let buffer = &self.buffers[view.buffer().index()];
        buffer[view.offset()..][..view.length()].into()
    }

    #[throws]
    fn read_file(&self, uri: &str) -> Vec<u8> {
        let path = self.resolve_uri(uri);
        trace!("Bundle::read_file: path = {}", path);
        std::fs::read(path)?
    }
}

#[throws]
fn read_data_uri(uri: &str) -> Option<Vec<u8>> {
    if !uri.starts_with("data:") { return None; }
    let data = uri.splitn(2, ',').nth(1).ok_or("invalid data URI")?;
    Some(base64::decode(data)?)
}

#[throws]
crate fn load_gltf(
    rloop: &mut RenderLoop,
    assets: &mut AssetCache,
    path: impl Into<String>,
) -> Scene {
    let bundle = &Bundle::import(path)?;
    let images = vec![None; bundle.document.images().len()];
    let mut loader = Loader { bundle, rloop, assets, images };
    load_scene(&mut loader)?
}

#[throws]
fn load_scene<'st, 'dat>(loader: &mut Loader<'st, 'dat>) -> Scene {
    let meshes = loader.bundle.document.meshes().map(|mesh| {
        load_mesh(loader, mesh)
    }).collect::<Result<Vec<_>>>()?;
    Scene { meshes }
}

#[throws]
fn load_mesh<'a, 'st, 'dat>(
    loader: &'a mut Loader<'st, 'dat>,
    mesh: gltf::Mesh<'dat>,
) -> Mesh {
    let primitives = mesh.primitives().map(|primitive| {
        load_primitive(loader, primitive)
    }).collect::<Result<Vec<_>>>()?;
    Mesh { primitives }
}

#[throws]
fn load_primitive<'a, 'st, 'dat>(
    loader: &'a mut Loader<'st, 'dat>,
    prim: gltf::Primitive<'dat>,
) -> Primitive {
    let mesh = load_primitive_mesh(loader, &prim)?;
    Primitive {
        bbox: get_bbox(&prim),
        material: load_material(loader, &mesh, prim.material())?,
        mesh,
    }
}

fn get_bbox(prim: &gltf::Primitive<'_>) -> BBox<f32, 3> {
    let bbox = prim.bounding_box();
    BBox::new(vec(bbox.min), vec(bbox.max))
}

#[throws]
fn load_primitive_mesh<'dat>(
    loader: &mut Loader<'_, 'dat>,
    prim: &gltf::Primitive<'dat>,
) -> Arc<RenderMesh> {
    tassert!(prim.mode() == mesh::Mode::Triangles,
        "unsupported primitive topology: {:?}", prim.mode());
    tassert!(!prim.attributes().is_empty(), "no attribute data");

    let mut attrs: PartialEnumMap<_, _> = Default::default();
    for (sem, accessor) in prim.attributes() {
        let attr = accessor_semantic(&sem)?;
        let (format, slice) = loader.bundle.read_attr_accessor(&accessor)?;
        attrs.insert(attr, (format, slice));
    }
    let index = tryopt!(loader.bundle.read_index_accessor(&prim.indices()?))
        .transpose()?;

    let mut builder = RenderMeshBuilder::from_loop(loader.rloop);
    builder.lifetime(Lifetime::Static);
    for (attr, (fmt, slice)) in attrs.drain() {
        builder.attr(attr, fmt, slice.data, slice.offset, slice.len);
    }
    if let Some((ty, slice)) = index {
        builder.index(ty, slice.data, slice.offset, slice.len);
    }
    Arc::new(builder.build())
}

#[throws]
fn accessor_semantic(sem: &gltf::Semantic) -> VertexAttr {
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
        _ => throw!("unsupported semantic"),
    }
}

#[throws]
fn accessor_format(acc: &gltf::Accessor<'_>) -> Format {
    use accessor::{DataType as Type, Dimensions as Dim};
    let tuple = (acc.data_type(), acc.dimensions(), acc.normalized());
    match tuple {
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
        _ => throw!(format!("unsupported format: {:?}", tuple)),
    }
}

#[throws]
fn accessor_index_type(ty: accessor::DataType) -> IndexType {
    use accessor::DataType;
    match ty {
        DataType::U16 => IndexType::U16,
        DataType::U32 => IndexType::U32,
        _ => throw!(format!("bad index type: {:?}", ty)),
    }
}

#[throws]
fn load_material<'a, 'st, 'dat>(
    loader: &'a mut Loader<'st, 'dat>,
    mesh: &Arc<RenderMesh>,
    material: gltf::Material<'dat>,
) -> Arc<MaterialDef> {
    // TODO: Actually implement shaders
    let vertex_shader = Arc::clone(&loader.rloop.shaders().static_vert);
    let vertex_shader: Arc<ShaderSpec> = Arc::new(vertex_shader.into());
    let frag_shader = Arc::clone(&loader.rloop.specs().albedo_frag);

    let vertex_layout = mesh.vertex_layout()
        .input_layout_for_shader(vertex_shader.shader());
    let image_bindings = load_material_images(loader, &material)?;

    let desc = MaterialDesc {
        vertex_layout,
        stages: partial_map! {
            ShaderStage::Vertex => vertex_shader,
            ShaderStage::Fragment => frag_shader,
        },
        image_bindings,
    };
    loader.rloop.define_material(&desc)
}

#[throws]
fn load_material_images<'a, 'st, 'dat>(
    loader: &'a mut Loader<'st, 'dat>,
    material: &gltf::Material<'dat>,
) -> MaterialImageBindings {
    tassert!(material.alpha_mode() == gltf::material::AlphaMode::Opaque,
        "transparency not supported");

    let normal = if let Some(binding) = material.normal_texture() {
        tassert!(binding.tex_coord() == 0, "texcoord != 0");
        tassert!(binding.scale() == 1.0, "normal scale != 1");
        Some(load_texture(loader, binding.texture())?)
    } else { None };

    let pbr = material.pbr_metallic_roughness();

    macro_rules! try_load_texture { ($texture:expr) => {
        if let Some(binding) = $texture {
            tassert!(binding.tex_coord() == 0, "texcoord != 0");
            Some(load_texture(loader, binding.texture())?)
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
fn load_texture<'a, 'st, 'dat>(
    loader: &'a mut Loader<'st, 'dat>,
    tex: gltf::texture::Texture<'dat>,
) -> ImageBindingDesc {
    let image = Arc::clone(get_image(loader, tex.source())?);
    ImageBindingDesc {
        subresources: image.all_subresources(),
        image,
        sampler_state: load_sampler(tex.sampler()),
    }
}

#[throws]
fn get_image<'a, 'st, 'dat>(
    loader: &'a mut Loader<'st, 'dat>,
    image: gltf::Image<'dat>,
) -> &'a Arc<ImageDef> {
    use gltf::image::Source;

    let index = image.index();
    let images = unsafe { crate::extend_lt(&loader.images) };
    let loaded = images.get(index)?;
    tryopt! { return Ok(loaded.as_ref()?) };

    let source = image.source();
    // TODO: Use a generic URI resolver instead of treating everything
    // as a file. Sometimes you want to load from (e.g.) a zip archive.
    let image = match source {
        Source::View { view, mime_type } => {
            let data = loader.bundle.get_view(&view)?;
            load_data_image(loader, &data, index, Some(mime_type))?
        },
        Source::Uri { uri, mime_type } =>
            if let Some(data) = read_data_uri(uri)? {
                load_data_image(loader, &data, index, mime_type)?
            } else {
                let src = &loader.bundle.resolve_uri(uri);
                Arc::clone(loader.assets.get_or_load_image(loader.rloop, src)?)
            },
    };

    loader.images[index] = Some(image);
    loader.images[index].as_ref().unwrap()
}

// TODO: Use mime type when available
#[throws]
fn load_data_image(
    loader: &mut Loader<'_, '_>,
    data: &[u8],
    index: usize,
    _mime: Option<&str>,
) -> Arc<ImageDef> {
    let image = image::load_from_memory(data)?;
    let name = format!("{}[image={}]", loader.bundle.source, index);
    load_image(&mut loader.rloop, image, Some(name))
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
