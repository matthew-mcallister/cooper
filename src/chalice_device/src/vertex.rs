use derivative::Derivative;

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

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct VertexInputLayout {
    pub topology: PrimitiveTopology,
    pub bindings: SmallVec<vk::VertexInputBindingDescription, 2>,
    pub attributes: SmallVec<vk::VertexInputAttributeDescription, 6>,
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
