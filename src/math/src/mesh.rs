//! Mesh generation for stock geometric types.

#[derive(Clone, Copy, Debug)]
pub struct UnitCube;

impl UnitCube {
    const INDEX_BUFFER: &[u32] = &[
        // x = 0
        1, 0, 2,
        1, 2, 3,
        // x = 1
        5, 4, 6,
        5, 6, 7,
        // y = 0
        9, 8, 10,
        9, 10, 11,
        // y = 1
        13, 12, 14,
        13, 14, 15,
        // z = 0
        17, 16, 18,
        17, 18, 19,
        // z = 1
        21, 20, 22,
        21, 22, 23,
    ];

    const VERTS: &[[[[f32; 3]; 2]; 2]; 2] = &[
        [
            [[-1.0, -1.0, -1.0], [-1.0, -1.0,  1.0]],
            [[-1.0,  1.0, -1.0], [-1.0,  1.0,  1.0]],
        ],
        [
            [[ 1.0, -1.0, -1.0], [ 1.0, -1.0,  1.0]],
            [[ 1.0,  1.0, -1.0], [ 1.0,  1.0,  1.0]],
        ],
    ];

    const POS_BUFFER: &[[f32; 3]] = &[
        // x = 0
        Self::VERTS[0][0][0], Self::VERTS[0][1][0], Self::VERTS[0][0][1], Self::VERTS[0][1][1],
        // x = 1
        Self::VERTS[1][0][0], Self::VERTS[1][0][1], Self::VERTS[1][1][0], Self::VERTS[1][1][1],
        // y = 0
        Self::VERTS[0][0][0], Self::VERTS[0][0][1], Self::VERTS[1][0][0], Self::VERTS[1][0][1],
        // y = 1
        Self::VERTS[0][1][0], Self::VERTS[1][1][0], Self::VERTS[0][1][1], Self::VERTS[1][1][1],
        // z = 0
        Self::VERTS[0][0][0], Self::VERTS[1][0][0], Self::VERTS[0][1][0], Self::VERTS[1][1][0],
        // z = 1
        Self::VERTS[0][0][1], Self::VERTS[0][1][1], Self::VERTS[1][0][1], Self::VERTS[1][1][1],
    ];

    const NORMALS: &[[f32; 3]] = &[
        [-1.0,  0.0,  0.0], [1.0, 0.0, 0.0],
        [ 0.0, -1.0,  0.0], [0.0, 1.0, 0.0],
        [ 0.0,  0.0, -1.0], [0.0, 0.0, 1.0],
    ];

    const NORMAL_BUFFER: &[[f32; 3]] = &[
        Self::NORMALS[0], Self::NORMALS[0], Self::NORMALS[0], Self::NORMALS[0],
        Self::NORMALS[1], Self::NORMALS[1], Self::NORMALS[1], Self::NORMALS[1],
        Self::NORMALS[2], Self::NORMALS[2], Self::NORMALS[2], Self::NORMALS[2],
        Self::NORMALS[3], Self::NORMALS[3], Self::NORMALS[3], Self::NORMALS[3],
        Self::NORMALS[4], Self::NORMALS[4], Self::NORMALS[4], Self::NORMALS[4],
        Self::NORMALS[5], Self::NORMALS[5], Self::NORMALS[5], Self::NORMALS[5],
    ];

    pub fn index_count(&self) -> u32 {
        36
    }

    pub fn vertex_count(&self) -> u32 {
        24
    }

    pub fn triangle_count(&self) -> u32 {
        12
    }

    pub fn index_buffer(&self) -> &[u32] {
        Self::INDEX_BUFFER
    }

    pub fn pos_buffer(&self) -> &[[f32; 3]] {
        Self::POS_BUFFER
    }

    pub fn normal_buffer(&self) -> &[[f32; 3]] {
        Self::NORMAL_BUFFER
    }
}
