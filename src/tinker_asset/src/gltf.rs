use std::path::{Path, PathBuf};
use std::sync::Arc;

use base::{partial_map, PartialEnumMap};
use derive_more::Constructor;
use device::*;
use fehler::{throw, throws};
use gfx::*;
use gltf::{accessor, mesh};
use log::trace;
use math::{vec, BBox3};

use crate::asset::*;
use crate::scene::*;
use crate::{create_monochrome_image, vec_to_bytes, Error, Result};

#[derive(Debug)]
struct Bundle {
    source: String,
    base: PathBuf,
    document: gltf::Document,
}

type Buffer = Arc<Vec<u8>>;

#[derive(Debug)]
struct Loader<'st, 'dat> {
    bundle: &'dat Bundle,
    buffers: &'dat [Buffer],
    rloop: &'st mut RenderLoop,
    assets: &'st mut AssetCache,
    images: Vec<Arc<ImageDef>>,
    meshes: Vec<Mesh>,
}

#[derive(Constructor, Debug)]
struct SharedSlice {
    data: Arc<Vec<u8>>,
    offset: usize,
    len: usize,
}

impl Bundle {
    #[throws]
    pub(crate) fn import(path: impl Into<String>) -> (Self, Option<Vec<u8>>) {
        let source = path.into();
        let file = std::fs::File::open(&source)?;
        let gltf::Gltf { document, blob } = gltf::Gltf::from_reader_without_validation(file)?;
        let base = Path::new(&source).parent()?.into();
        (
            Self {
                base,
                source,
                document,
            },
            blob,
        )
    }

    fn resolve_uri(&self, uri: &str) -> String {
        self.base.join(uri).into_os_string().into_string().unwrap()
    }

    #[throws]
    fn read_file(&self, uri: &str) -> Vec<u8> {
        let path = self.resolve_uri(uri);
        trace!("Bundle::read_file: path = {}", path);
        std::fs::read(path)?
    }
}

impl<'st, 'dat> Loader<'st, 'dat> {
    #[throws]
    fn accessor_view_data(&self, accessor: &gltf::Accessor<'_>) -> (&'dat Arc<Vec<u8>>, usize) {
        let view = accessor.view().ok_or("sparse accessor")?;
        // TODO: remove all potential panics on slices
        (&self.buffers[view.buffer().index()], view.offset())
    }

    // TODO: prevent creating redundant copies in VRAM
    #[throws]
    fn read_attr_accessor(&self, accessor: &gltf::Accessor<'_>) -> (Format, SharedSlice) {
        let format = accessor_format(accessor)?;
        let (data, offset) = self.accessor_view_data(accessor)?;
        let len = accessor.count() * format.size();
        (format, SharedSlice::new(Arc::clone(data), offset, len))
    }

    // TODO: redundant copies
    #[throws]
    fn read_index_accessor(&self, accessor: &gltf::Accessor<'_>) -> (IndexType, SharedSlice) {
        use gltf::accessor::DataType;
        let (data, offset) = self.accessor_view_data(accessor)?;
        let acc_ty = accessor.data_type();
        let idx_ty = match acc_ty {
            DataType::U8 => {
                // Convert U8 to U16
                let len = accessor.count();
                let old_data = &data[offset..][..len];
                let new_data = old_data.iter().map(|&x| x as u16).collect();
                let new_data = vec_to_bytes(new_data);
                let new_len = new_data.len();
                let shared = SharedSlice::new(Arc::new(new_data), 0, new_len);
                return (IndexType::U16, shared);
            }
            DataType::U16 => IndexType::U16,
            DataType::U32 => IndexType::U32,
            _ => throw!(format!("unsupported index type: {:?}", acc_ty)),
        };
        let len = accessor.count() * idx_ty.size();
        (idx_ty, SharedSlice::new(Arc::clone(data), offset, len))
    }

    #[throws]
    fn get_view(&self, view: &gltf::buffer::View<'_>) -> &'dat [u8] {
        let buffer = &self.buffers[view.buffer().index()];
        buffer[view.offset()..][..view.length()].into()
    }

    fn into_resources(self) -> SceneResources {
        SceneResources {
            meshes: self.meshes,
        }
    }
}

#[throws]
pub(crate) fn load_gltf(
    rloop: &mut RenderLoop,
    assets: &mut AssetCache,
    path: impl Into<String>,
) -> SceneCollection {
    let (bundle, blob) = Bundle::import(path)?;
    let buffers = load_buffers(&bundle, blob)?;
    let resources = load_resources(rloop, assets, &bundle, &buffers)?;
    let nodes = load_nodes(bundle.document.nodes())?;
    let scenes = bundle
        .document
        .scenes()
        .map(load_scene)
        .collect::<Result<Vec<_>>>()?;
    tassert!(!scenes.is_empty(), "no scenes in document");
    let default_scene_idx = bundle
        .document
        .default_scene()
        .map_or(0, |scene| scene.index());
    SceneCollection {
        resources,
        nodes,
        scenes,
        default_scene_idx,
    }
}

#[throws]
fn load_resources(
    rloop: &mut RenderLoop,
    assets: &mut AssetCache,
    bundle: &Bundle,
    buffers: &[Buffer],
) -> SceneResources {
    let mut loader = Loader {
        rloop,
        assets,
        bundle,
        buffers,
        images: Vec::new(),
        meshes: Vec::new(),
    };

    loader.images = loader
        .bundle
        .document
        .images()
        .map(|image| load_image(&mut loader, image))
        .collect::<Result<_>>()?;
    loader.meshes = loader
        .bundle
        .document
        .meshes()
        .map(|mesh| load_mesh(&mut loader, mesh))
        .collect::<Result<_>>()?;

    loader.into_resources()
}

#[throws]
fn load_buffers(bundle: &Bundle, blob: Option<Vec<u8>>) -> Vec<Buffer> {
    use gltf::buffer::Source;
    let blob = tryopt!(Arc::new(blob?));
    bundle
        .document
        .buffers()
        .map(|buf| {
            Ok(match buf.source() {
                Source::Bin => Arc::clone(blob.as_ref()?),
                Source::Uri(uri) => {
                    if let Some(data) = read_data_uri(uri)? {
                        Arc::new(data)
                    } else {
                        Arc::new(bundle.read_file(uri)?)
                    }
                }
            })
        })
        .collect::<Result<_>>()?
}

#[throws]
fn read_data_uri(uri: &str) -> Option<Vec<u8>> {
    if !uri.starts_with("data:") {
        return None;
    }
    let data = uri.splitn(2, ',').nth(1).ok_or("invalid data URI")?;
    Some(base64::decode(data)?)
}

#[throws]
fn load_image<'st, 'dat>(
    loader: &mut Loader<'st, 'dat>,
    image: gltf::Image<'dat>,
) -> Arc<ImageDef> {
    use gltf::image::Source;

    // TODO: Use a generic URI resolver instead of treating everything
    // as a file. Sometimes you want to load from (e.g.) a zip archive.
    let index = image.index();
    match image.source() {
        Source::View { view, mime_type } => {
            let data = loader.get_view(&view)?;
            load_data_image(loader, &data, index, Some(mime_type))?
        }
        Source::Uri { uri, mime_type } => {
            if let Some(data) = read_data_uri(uri)? {
                load_data_image(loader, &data, index, mime_type)?
            } else {
                let src = &loader.bundle.resolve_uri(uri);
                Arc::clone(loader.assets.get_or_load_image(loader.rloop, src)?)
            }
        }
    }
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
    crate::load_image(&mut loader.rloop, image, Some(name))
}

#[throws]
fn load_mesh<'a, 'st, 'dat>(loader: &'a mut Loader<'st, 'dat>, mesh: gltf::Mesh<'dat>) -> Mesh {
    let primitives = mesh
        .primitives()
        .map(|primitive| load_primitive(loader, primitive))
        .collect::<Result<Vec<_>>>()?;
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

fn get_bbox(prim: &gltf::Primitive<'_>) -> BBox3 {
    let bbox = prim.bounding_box();
    BBox3::new(vec(bbox.min), vec(bbox.max))
}

#[throws]
fn load_primitive_mesh<'dat>(
    loader: &mut Loader<'_, 'dat>,
    prim: &gltf::Primitive<'dat>,
) -> Arc<RenderMesh> {
    tassert!(
        prim.mode() == mesh::Mode::Triangles,
        "unsupported primitive topology: {:?}",
        prim.mode()
    );
    tassert!(!prim.attributes().is_empty(), "no attribute data");

    let mut attrs: PartialEnumMap<_, _> = Default::default();
    for (sem, accessor) in prim.attributes() {
        let attr = accessor_semantic(&sem)?;
        let (format, slice) = loader.read_attr_accessor(&accessor)?;
        attrs.insert(attr, (format, slice));
    }
    let index = tryopt!(loader.read_index_accessor(&prim.indices()?)).transpose()?;

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

// TODO: Actually implement all required shaders
#[throws]
fn load_material<'a, 'st, 'dat>(
    loader: &'a mut Loader<'st, 'dat>,
    mesh: &Arc<RenderMesh>,
    material: gltf::Material<'dat>,
) -> MaterialDesc {
    let vertex_shader = if mesh.bindings().contains_key(VertexAttr::Texcoord0) {
        Arc::clone(&loader.rloop.shaders().static_vert)
    } else {
        Arc::clone(&loader.rloop.shaders().minimal_vert)
    };
    let vertex_shader: Arc<ShaderSpec> = Arc::new(vertex_shader.into());

    let frag_shader = Arc::clone(&loader.rloop.specs().albedo_frag);

    let vertex_layout = mesh
        .vertex_layout()
        .input_layout_for_shader(vertex_shader.shader());
    let image_bindings = load_material_images(loader, &material)?;

    let cull_mode = match material.double_sided() {
        true => CullMode::None,
        false => CullMode::Back,
    };

    MaterialDesc {
        vertex_layout,
        stages: partial_map! {
            ShaderStage::Vertex => vertex_shader,
            ShaderStage::Fragment => frag_shader,
        },
        image_bindings,
        cull_mode,
    }
}

fn default_sampler() -> SamplerDesc {
    SamplerDesc {
        mag_filter: Filter::Nearest,
        min_filter: Filter::Nearest,
        mipmap_mode: SamplerMipmapMode::Nearest,
        anisotropy_level: AnisotropyLevel::One,
        ..Default::default()
    }
}

fn sfloat4_to_unorm4([r, g, b, a]: [f32; 4]) -> [u8; 4] {
    fn f(x: f32) -> u8 {
        (x * 255.0 + 0.5) as u8
    }
    [f(r), f(g), f(b), f(a)]
}

#[throws]
fn load_material_images<'a, 'st, 'dat>(
    loader: &'a mut Loader<'st, 'dat>,
    material: &gltf::Material<'dat>,
) -> MaterialImageBindings {
    tassert!(
        material.alpha_mode() == gltf::material::AlphaMode::Opaque,
        "transparency not supported"
    );

    if let Some(binding) = material.normal_texture() {
        tassert!(binding.scale() == 1.0, "normal texture scale != 1");
    }

    macro_rules! try_load_texture(($texture:expr) => {
        if let Some(binding) = $texture {
            tassert!(binding.tex_coord() == 0, "texcoord != 0");
            Some(load_texture(loader, binding.texture()))
        } else { None }
    });

    let normal = try_load_texture!(material.normal_texture()).unwrap_or_else(|| ImageBindingDesc {
        image: Arc::clone(loader.assets.default_normal_map()),
        sampler_state: default_sampler(),
    });

    let pbr = material.pbr_metallic_roughness();

    let base = sfloat4_to_unorm4(pbr.base_color_factor());
    let albedo = try_load_texture!(pbr.base_color_texture()).unwrap_or_else(|| ImageBindingDesc {
        image: create_monochrome_image(&mut loader.rloop, base),
        sampler_state: default_sampler(),
    });

    let (m, r) = (pbr.metallic_factor(), pbr.roughness_factor());
    let base = sfloat4_to_unorm4([m, r, 0.0, 0.0]);
    let metal_rough =
        try_load_texture!(pbr.metallic_roughness_texture()).unwrap_or_else(|| ImageBindingDesc {
            image: create_monochrome_image(&mut loader.rloop, base),
            sampler_state: default_sampler(),
        });

    let images = partial_map! {
        MaterialImage::Normal => normal,
        MaterialImage::Albedo => albedo,
        MaterialImage::MetallicRoughness => metal_rough,
    };

    images
}

fn load_texture<'a, 'st, 'dat>(
    loader: &'a mut Loader<'st, 'dat>,
    tex: gltf::texture::Texture<'dat>,
) -> ImageBindingDesc {
    let image = Arc::clone(&loader.images[tex.source().index() as usize]);
    ImageBindingDesc {
        image,
        sampler_state: load_sampler(tex.sampler()),
    }
}

fn load_sampler(sampler: gltf::texture::Sampler<'_>) -> SamplerDesc {
    let mag_filter = tryopt!(mag_filter(sampler.mag_filter()?)).unwrap_or(Filter::Linear);
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

fn min_filter(filter: gltf::texture::MinFilter) -> (Filter, SamplerMipmapMode) {
    use gltf::texture::MinFilter;
    match filter {
        MinFilter::Nearest | MinFilter::NearestMipmapNearest => {
            (Filter::Nearest, SamplerMipmapMode::Nearest)
        }
        MinFilter::LinearMipmapNearest => (Filter::Linear, SamplerMipmapMode::Nearest),
        MinFilter::NearestMipmapLinear => (Filter::Nearest, SamplerMipmapMode::Linear),
        MinFilter::Linear | MinFilter::LinearMipmapLinear => {
            (Filter::Linear, SamplerMipmapMode::Linear)
        }
    }
}

fn wrapping_mode(wrapping_mode: gltf::texture::WrappingMode) -> SamplerAddressMode {
    use gltf::texture::WrappingMode;
    match wrapping_mode {
        WrappingMode::ClampToEdge => SamplerAddressMode::ClampToEdge,
        WrappingMode::MirroredRepeat => SamplerAddressMode::MirroredRepeat,
        WrappingMode::Repeat => SamplerAddressMode::Repeat,
    }
}

#[throws]
fn load_nodes(node_list: gltf::iter::Nodes<'_>) -> Vec<Node> {
    let mut nodes = node_list
        .clone()
        .map(load_node)
        .collect::<Result<Vec<_>>>()?;
    assign_parents(node_list, &mut nodes[..])?;
    nodes
}

#[throws]
fn load_node(node: gltf::Node<'_>) -> Node {
    let transform = node.transform().into();
    let data = NodeData::from_node(&node);
    Node {
        transform,
        data,
        ..Default::default()
    }
}

#[throws]
fn assign_parents(src: gltf::iter::Nodes<'_>, dst: &mut [Node]) {
    for node in src {
        for child in node.children() {
            dst.get_mut(child.index() as usize)?.parent = Some(node.index() as u32);
        }
    }
}

#[throws]
fn load_scene(scene: gltf::Scene<'_>) -> Scene {
    let nodes = scene.nodes().map(|node| node.index() as u32).collect();
    Scene { nodes }
}
