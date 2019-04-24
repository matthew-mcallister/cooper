#![allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum VertexAttrs {
    PosNormTex0Tan,
    PosNormTex0TanJoint,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VertexPnxt {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coord_0: [f32; 2],
    pub tangent: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VertexPnxtj {
    pub pnxt: VertexPnxt,
    pub joints: [u8; 4],
    pub weights: [u8; 4],
}
