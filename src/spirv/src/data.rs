use derivative::Derivative;
use fnv::FnvHashMap as HashMap;
use rspirv::dr::ModuleHeader;
use spirv_headers as spv;

#[derive(Debug)]
pub struct Module {
    crate header: ModuleHeader,
    crate variables: HashMap<u32, Variable>,
    crate uniforms: HashMap<u32, Uniform>,
    crate entry_points: HashMap<String, EntryPoint>,
    crate decorations: HashMap<u32, DecorationSet>,
}

// Intermediate type used during module construction and discarded
// afterward.
#[derive(Debug, Default)]
crate struct DecorationSet {
    crate name: Option<String>,
    crate location: Option<u32>,
    crate set: Option<u32>,
    crate binding: Option<u32>,
}

#[derive(Debug, Derivative)]
#[derivative(Default)]
crate struct Variable {
    #[derivative(Default(value = "spv::StorageClass::UniformConstant"))]
    crate storage_class: spv::StorageClass,
    crate location: u32,
    crate name: Option<String>,
}

#[derive(Debug, Derivative)]
#[derivative(Default)]
crate struct Uniform {
    #[derivative(Default(value = "spv::StorageClass::UniformConstant"))]
    crate storage_class: spv::StorageClass,
    crate set: u32,
    crate binding: u32,
    crate name: Option<String>,
}

#[derive(Debug, Derivative)]
#[derivative(Default)]
crate struct EntryPoint {
    #[derivative(Default(value = "spv::ExecutionModel::Vertex"))]
    crate execution_model: spv::ExecutionModel,
    crate inputs: Vec<u32>,
    crate outputs: Vec<u32>,
}
