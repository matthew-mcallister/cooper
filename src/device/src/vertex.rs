use std::convert::TryInto;

use base::PartialEnumMap;
use derivative::Derivative;
use enum_map::Enum;

use crate::*;

wrap_vk_enum! {
    #[derive(Derivative)]
    #[derivative(Default)]
    pub enum IndexType {
        #[derivative(Default)]
        U16 = UINT16,
        U32 = UINT32,
    }
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

wrap_vk_enum! {
    #[derive(Derivative)]
    #[derivative(Default)]
    pub enum PrimitiveTopology {
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VertexStreamLayout {
    pub topology: PrimitiveTopology,
    pub attributes: PartialEnumMap<VertexAttr, VertexStreamAttr>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VertexStreamAttr {
    pub format: Format,
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct VertexInputLayout {
    pub topology: PrimitiveTopology,
    pub attributes: SmallVec<VertexAttributeBinding, 6>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct VertexAttributeBinding {
    pub location: u32,
    pub attribute: VertexAttr,
    pub format: Format,
}

impl IndexType {
    #[inline]
    pub fn size(self) -> usize {
        match self {
            Self::U16 => 2,
            Self::U32 => 4,
        }
    }
}

impl VertexStreamLayout {
    pub fn input_layout_for_shader(&self, shader: &Shader) -> VertexInputLayout {
        assert_eq!(shader.stage(), ShaderStage::Vertex);
        let mut attrs = SmallVec::with_capacity(shader.inputs().len());
        for &location in shader.inputs().iter() {
            let name: VertexAttr = location.try_into().unwrap();
            let attr = *self
                .attributes
                .get(name)
                .unwrap_or_else(|| panic!("missing attribute: {:?}", name));
            attrs.push(VertexAttributeBinding {
                location,
                attribute: name,
                format: attr.format,
            });
        }
        VertexInputLayout {
            topology: self.topology,
            attributes: attrs,
        }
    }
}

impl VertexInputLayout {
    pub(super) fn vk_bindings(&self) -> Vec<vk::VertexInputBindingDescription> {
        self.attributes
            .iter()
            .enumerate()
            .map(|(i, attr)| vk::VertexInputBindingDescription {
                binding: i as _,
                stride: attr.format.size() as _,
                input_rate: vk::VertexInputRate::VERTEX,
            })
            .collect()
    }

    pub(super) fn vk_attrs(&self) -> Vec<vk::VertexInputAttributeDescription> {
        self.attributes
            .iter()
            .enumerate()
            .map(|(i, attr)| vk::VertexInputAttributeDescription {
                location: attr.location,
                binding: i as _,
                format: attr.format.into(),
                offset: 0,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;
    use crate::*;
    use base::partial_map;

    unsafe fn smoke_test(vars: testing::TestVars) {
        use VertexAttr as Attr;

        let attr = |format| VertexStreamAttr { format };
        let layout = VertexStreamLayout {
            attributes: partial_map! {
                Attr::Position  => attr(Format::RGB32F),
                Attr::Normal    => attr(Format::RGB32F),
                Attr::Texcoord0 => attr(Format::RG16),
                Attr::Color     => attr(Format::RGB8),
                Attr::Joints    => attr(Format::RGBA8U),
                Attr::Weights   => attr(Format::RGBA8),
            },
            ..Default::default()
        };

        let shaders = TestShaders::new(vars.device());
        let _input = layout.input_layout_for_shader(&shaders.static_vert);
    }
}
