use std::sync::Arc;

use derivative::Derivative;
use gfx::{MaterialDesc, RenderMesh};
use gltf::scene::Transform as GltfTransform;
use math::{BBox, Matrix4, Vector3, Vector4, mat4, vec3};

#[derive(Debug)]
pub struct SceneCollection {
    pub resources: SceneResources,
    pub nodes: Vec<Node>,
    pub scenes: Vec<Scene>,
    pub default_scene_idx: usize,
}

#[derive(Debug)]
pub struct SceneResources {
    pub meshes: Vec<Mesh>,
}

#[derive(Debug)]
pub struct Mesh {
    pub primitives: Vec<Primitive>,
}

#[derive(Debug)]
pub struct Primitive {
    pub bbox: BBox<f32, 3>,
    pub mesh: Arc<RenderMesh>,
    pub material: MaterialDesc,
}

#[derive(Debug, Default)]
pub struct Scene {
    pub nodes: Vec<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct Node {
    pub parent: Option<u32>,
    pub transform: Transform,
    pub data: NodeData,
}

#[derive(Clone, Copy, Debug, Derivative)]
#[derivative(Default)]
pub enum Transform {
    #[derivative(Default)]
    Matrix(Matrix4<f32>),
    Decomposed {
        translation: Vector3<f32>,
        // TODO: Quaternions
        rotation: Vector4<f32>,
        scale: Vector3<f32>,
    },
}

#[derive(Clone, Debug, Derivative)]
#[derivative(Default)]
pub enum NodeData {
    #[derivative(Default)]
    Empty,
    Mesh(u32),
}

impl From<GltfTransform> for Transform {
    fn from(transform: GltfTransform) -> Self {
        match transform {
            GltfTransform::Matrix { matrix } => Self::Matrix(matrix.into()),
            GltfTransform::Decomposed { translation, rotation, scale } =>
                Self::Decomposed {
                    translation: translation.into(),
                    rotation: rotation.into(),
                    scale: scale.into(),
                },
        }
    }
}

fn xform_matrix(p: Vector3<f32>, q: Vector4<f32>, s: Vector3<f32>) ->
    Matrix4<f32>
{
    let (v, w) = (q.xyz(), q[3]);
    let v2 = v * v * 2.0;
    let vij = vec3(v[1] * v[2], v[2] * v[0], v[0] * v[1]) * 2.0;
    let vw = v * w * 2.0;
    let r = [
        vec3(1.0 - v2[1] - v2[2], vij[2] + vw[2], vij[1] - vw[1]),
        vec3(vij[2] - vw[2], 1.0 - v2[2] - v2[0], vij[0] + vw[0]),
        vec3(vij[1] + vw[1], vij[0] - vw[0], 1.0 - v2[0] - v2[1]),
    ];
    mat4(
        r[0].xyz0() * s[0],
        r[1].xyz0() * s[1],
        r[2].xyz0() * s[2],
        p.xyz1(),
    )
}

impl From<Transform> for Matrix4<f32> {
    fn from(transform: Transform) -> Self {
        match transform {
            Transform::Matrix(matrix) => matrix.into(),
            Transform::Decomposed { translation, rotation, scale } =>
                xform_matrix(translation, rotation, scale),
        }
    }
}

impl NodeData {
    crate fn from_node(node: &gltf::Node<'_>) -> Self {
        if let Some(mesh) = node.mesh() {
            Self::Mesh(mesh.index() as u32)
        } else {
            Self::Empty
        }
    }
}

impl SceneCollection {
    pub fn default_scene(&self) -> &Scene {
        &self.scenes[self.default_scene_idx]
    }

    /// Calculates the base world transforms of each node in the
    /// collection according to the node hierarchy.
    pub fn world_xforms(&self) -> Vec<Matrix4<f32>> {
        // TODO: Real implementation
        self.nodes.iter().map(|node| node.transform.into()).collect()
    }
}
