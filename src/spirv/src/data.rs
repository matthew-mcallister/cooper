use derivative::Derivative;
use fnv::FnvHashMap as HashMap;
use rspirv::dr::ModuleHeader;
use spirv_headers as spv;

#[derive(Debug)]
pub struct Module {
    pub(crate) header: ModuleHeader,
    pub(crate) variables: HashMap<u32, Variable>,
    pub(crate) uniforms: HashMap<u32, Uniform>,
    pub(crate) entry_points: HashMap<String, EntryPoint>,
    pub(crate) decorations: HashMap<u32, DecorationSet>,
}

// Intermediate type used during module construction and discarded
// afterward.
#[derive(Debug, Default)]
pub(crate) struct DecorationSet {
    pub(crate) name: Option<String>,
    pub(crate) location: Option<u32>,
    pub(crate) set: Option<u32>,
    pub(crate) binding: Option<u32>,
}

#[derive(Debug, Derivative)]
#[derivative(Default)]
pub(crate) struct Variable {
    #[derivative(Default(value = "spv::StorageClass::UniformConstant"))]
    pub(crate) storage_class: spv::StorageClass,
    pub(crate) location: u32,
    pub(crate) name: Option<String>,
}

#[derive(Debug, Derivative)]
#[derivative(Default)]
pub(crate) struct Uniform {
    #[derivative(Default(value = "spv::StorageClass::UniformConstant"))]
    pub(crate) storage_class: spv::StorageClass,
    pub(crate) set: u32,
    pub(crate) binding: u32,
    pub(crate) name: Option<String>,
}

#[derive(Debug, Derivative)]
#[derivative(Default)]
pub(crate) struct EntryPoint {
    #[derivative(Default(value = "spv::ExecutionModel::Vertex"))]
    pub(crate) execution_model: spv::ExecutionModel,
    pub(crate) inputs: Vec<u32>,
    pub(crate) outputs: Vec<u32>,
}
