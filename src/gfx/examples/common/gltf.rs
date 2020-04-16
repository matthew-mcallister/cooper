use anyhow::{self as any, anyhow, Context, Error};
use cooper_gfx::*;
use fehler::{throw, throws};
use gltf::{accessor, mesh};

#[derive(Debug)]
crate struct GltfBundle {
    crate path: String,
    crate document: gltf::Document,
    crate buffers: Vec<gltf::buffer::Data>,
    crate images: Vec<gltf::image::Data>,
}

#[derive(Debug)]
crate struct Mesh {
    crate render_mesh: RenderMesh,
}

impl GltfBundle {
    crate fn import(path: impl Into<String>) -> gltf::Result<Self> {
        let path = path.into();
        let (document, buffers, images) = gltf::import(&path)?;
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
    crate fn from_gltf(rl: &RenderLoop, bundle: &GltfBundle) ->
        any::Result<Self>
    {
        let prim: Option<_> = try {
            bundle.document.meshes().next()?.primitives().next()?
        };
        let prim = prim.ok_or_else(|| anyhow!("no primitives: path: `{}`"))?;
        Self::from_primitive(rl, bundle, &prim)
    }

    crate fn from_primitive(
        rl: &RenderLoop,
        bundle: &GltfBundle,
        prim: &gltf::Primitive<'_>,
    ) -> any::Result<Self> {
        // TODO: would like an easier way to attach this info
        // TODO: why is there no way to get the parent mesh index?!
        Self::from_prim_inner(rl, bundle, prim).context(format!(
            "path: `{}`, primitive: {}", bundle.path, prim.index()))
    }

    #[throws]
    fn from_prim_inner(
        rl: &RenderLoop,
        bundle: &GltfBundle,
        prim: &gltf::Primitive<'_>,
    ) -> Self {
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
        builder.lifetime(Lifetime::Static).vertex_count(vertex_count as _);
        for &(attr, fmt, data) in attrs.iter() {
            builder.attr(attr, fmt, data);
        }
        if let Some((ty, buf)) = index {
            builder.index(ty, buf);
        }
        let render_mesh = unsafe { builder.build() };

        Self { render_mesh }
    }
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
