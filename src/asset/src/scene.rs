use std::sync::Arc;

use derivative::Derivative;
use gfx::{MaterialDesc, RenderMesh};
use gltf::scene::Transform as GltfTransform;
use math::{BBox3, Matrix4, Quaternion, Vector3};

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
    pub bbox: BBox3,
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
    Matrix(Matrix4),
    Decomposed {
        translation: Vector3,
        rotation: Quaternion,
        scale: Vector3,
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

impl From<Transform> for Matrix4 {
    fn from(transform: Transform) -> Self {
        match transform {
            Transform::Matrix(matrix) => matrix,
            Transform::Decomposed { translation, rotation, scale } =>
                rotation.to_matrix().scale(scale).translate(translation),
        }
    }
}

impl Transform {
    pub fn to_matrix(self) -> Matrix4 {
        self.into()
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
    pub fn world_xforms(&self) -> Vec<Matrix4> {
        // TODO: Real implementation
        self.nodes.iter().map(|node| node.transform.into()).collect()
    }
}
