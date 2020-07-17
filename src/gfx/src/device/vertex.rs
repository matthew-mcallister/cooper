use std::convert::TryInto;

use base::PartialEnumMap;
use derivative::Derivative;
use enum_map::Enum;

use crate::*;

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
crate struct VertexLayout {
    crate topology: PrimitiveTopology,
    crate packing: VertexPacking,
    crate attrs: PartialEnumMap<VertexAttr, VertexLayoutAttr>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
crate struct VertexLayoutAttr {
    crate format: Format,
}

/// A limited choice of schemes for storing vertex attributes in memory.
#[derive(Clone, Copy, Debug, Derivative, Eq, Hash, PartialEq)]
#[derivative(Default)]
crate enum VertexPacking {
    /// Each vertex attribute is stored in a separate buffer.
    #[derivative(Default)]
    Unpacked,
    // All vertex attributes are stored in a single buffer.
    //Packed,
}

wrap_vk_enum! {
    #[derive(Derivative)]
    #[derivative(Default)]
    crate enum PrimitiveTopology {
        PointList = POINT_LIST,
        LineList = LINE_LIST,
        LineStrip = LINE_STRIP,
        #[derivative(Default)]
        TriangleList = TRIANGLE_LIST,
        TriangleStrip = TRIANGLE_STRIP,
        TriangleFan = TRIANGLE_FAN,
        LineListWithAdjacency = LINE_LIST_WITH_ADJACENCY,
        LineStripWithAdjacency = LINE_STRIP_WITH_ADJACENCY,
        TriangleListWithAdjacency = TRIANGLE_LIST_WITH_ADJACENCY,
        TriangleStripWithAdjacency = TRIANGLE_STRIP_WITH_ADJACENCY,
        PatchList = PATCH_LIST,
    }
}

/// Opaque object used in pipeline creation and vertex buffer binding.
// TODO: maybe it would have been fine to use fixed vertex buffer
// positions and bind dummy buffers for attributes not present
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
crate struct VertexInputLayout {
    pub(super) topology: PrimitiveTopology,
    pub(super) packing: VertexPacking,
    pub(super) attrs: PartialEnumMap<VertexAttr, VertexInputAttr>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct VertexInputAttr {
    pub(super) location: u32,
    pub(super) format: Format,
}

primitive_enum! {
    @[try_from: u8, u16, u32, u64, usize]
    @[try_from_error: &'static str = "not a valid vertex attribute"]
    @[into: u8, u16, u32, u64, usize]
    #[derive(Clone, Copy, Debug, Enum, Eq, Hash, PartialEq)]
    pub enum VertexAttr {
        Position = 0,
        Normal = 1,
        Tangent = 2,
        QTangent = 3,
        Texcoord0 = 4,
        Texcoord1 = 5,
        Color = 6,
        Joints = 7,
        Weights = 8,
        Velocity = 9,
    }
}

// TODO: Index buffer!
#[derive(Clone, Copy, Debug)]
crate enum VertexData<'a> {
    Unpacked(PartialEnumMap<VertexAttr, BufferRange<'a>>),
}

wrap_vk_enum! {
    #[derive(Derivative)]
    #[derivative(Default)]
    pub enum IndexType {
        #[derivative(Default)]
        U16 = UINT16,
        U32 = UINT32,
    }
}

impl IndexType {
    pub fn size(self) -> usize {
        match self {
            Self::U16 => 2,
            Self::U32 => 4,
        }
    }
}

impl VertexLayout {
    crate fn to_input_layout(&self, shader: &Shader) -> VertexInputLayout {
        VertexInputLayout::new(self, shader)
    }
}

impl VertexInputLayout {
    crate fn new(layout: &VertexLayout, shader: &Shader) -> Self {
        assert_eq!(shader.stage(), ShaderStage::Vertex);
        let mut attrs = PartialEnumMap::new();
        for &location in shader.inputs().iter() {
            let name = location.try_into().unwrap();
            let attr = &layout.attrs.get(name).unwrap_or_else(||
                panic!("missing attribute: {:?}", name));
            assert!(!attrs.contains_key(name));
            attrs.insert(name, VertexInputAttr {
                location,
                format: attr.format,
            });
        }

        let packing = if shader.inputs().len() < 2 { VertexPacking::Unpacked }
            else { layout.packing };

        Self {
            topology: layout.topology,
            packing,
            attrs,
        }
    }

    pub(super) fn vk_bindings(&self) -> Vec<vk::VertexInputBindingDescription>
    {
        match self.packing {
            VertexPacking::Unpacked => self.attrs.values()
                .enumerate()
                .map(|(i, attr)| vk::VertexInputBindingDescription {
                    binding: i as _,
                    stride: attr.format.size() as _,
                    input_rate: vk::VertexInputRate::VERTEX,
                })
                .collect(),
        }
    }

    pub(super) fn vk_attrs(&self) -> Vec<vk::VertexInputAttributeDescription> {
        match self.packing {
            VertexPacking::Unpacked => self.attrs.values()
                .enumerate()
                .map(|(i, attr)| vk::VertexInputAttributeDescription {
                    location: attr.location,
                    binding: i as _,
                    format: attr.format.into(),
                    offset: 0,
                })
                .collect(),
        }
    }
}

impl<'a> VertexData<'a> {
    pub(super) fn map_bindings<'b>(&'b self, layout: &'b VertexInputLayout) ->
        impl Iterator<Item = BufferRange<'a>> + 'b
    {
        match self {
            Self::Unpacked(data) => layout.attrs.keys().map(move |k| data[k])
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use base::partial_map;
    use crate::*;
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        use VertexAttr as Attr;

        let state = SystemState::new(Arc::clone(vars.device()));
        let globals = Globals::new(&state);

        let attr = |format| VertexLayoutAttr { format };
        let layout = VertexLayout {
            attrs: partial_map! {
                Attr::Position  => attr(Format::RGB32F),
                Attr::Normal    => attr(Format::RGB32F),
                Attr::Texcoord0 => attr(Format::RG16),
                Attr::Color     => attr(Format::RGB8),
                Attr::Joints    => attr(Format::RGBA8U),
                Attr::Weights   => attr(Format::RGBA8),
            },
            ..Default::default()
        };
        let _input = layout.to_input_layout(&globals.shaders.static_vert);
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
