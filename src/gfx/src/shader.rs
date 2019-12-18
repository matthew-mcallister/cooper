use std::ffi::CStr;
use std::ptr;
use std::sync::Arc;

use base::Name;
use derivative::Derivative;
use derive_more::Constructor;
use enum_map::Enum;
use fnv::FnvHashMap;
use prelude::*;

use crate::*;

#[derive(Debug)]
crate struct Shader {
    device: Arc<Device>,
    module: vk::ShaderModule,
    code: Vec<u8>,
    stage: ShaderStage,
    inputs: Vec<ShaderVar>,
    outputs: Vec<ShaderVar>,
    set_bindings: Vec<ShaderSetBinding>,
    spec_consts: FnvHashMap<Name, SpecConstDecl>,
}

/// A choice of shader plus specialization constant values.
#[derive(Debug, Derivative)]
#[derivative(Hash, PartialEq)]
crate struct ShaderSpec {
    #[derivative(Hash(hash_with="arc_ptr_hash"))]
    #[derivative(PartialEq(compare_with="std::sync::Arc::ptr_eq"))]
    shader: Arc<Shader>,
    spec_info: vk::SpecializationInfo,
    spec_map: Vec<vk::SpecializationMapEntry>,
    data: Vec<u8>,
}
impl Eq for ShaderSpec {}

#[derive(Clone, Copy, Debug)]
crate struct SpecConstDecl {
    // TODO: Name length limitation bites
    crate name: Name,
    crate id: u32,
    crate size: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
crate enum ShaderScalar { Int, Uint, Float, Double }

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
crate enum ShaderDim { One, Two, Three, Four }

/// The GLSL type of a shader interface (`in` or `out`) variable.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
crate struct ShaderVarType {
    crate scalar: ShaderScalar,
    crate dim: ShaderDim,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
crate struct ShaderVar {
    crate location: u32,
    crate ty: ShaderVarType,
    /// Optional name for mapping inputs to vertex attributes.
    crate attr: Option<VertexAttrName>,
    /// Optional name for mapping outputs to subpass attachments. If
    /// specialization constants are configured, this information could
    /// be used to automatically map shader outputs to attachments.
    crate attch: Option<AttachmentName>,
}

#[derive(Clone, Constructor, Debug, Derivative)]
#[derivative(Hash, PartialEq)]
crate struct ShaderSetBinding {
    crate index: u32,
    #[derivative(Hash(hash_with="arc_ptr_hash"))]
    #[derivative(PartialEq(compare_with="std::sync::Arc::ptr_eq"))]
    crate layout: Arc<DescriptorSetLayout>,
}
impl Eq for ShaderSetBinding {}

#[derive(Clone, Copy, Debug, Enum, Eq, Hash, PartialEq)]
crate enum ShaderStage {
    Vertex,
    TessControl,
    TessEval,
    Geometry,
    Fragment,
    Compute,
}

impl Drop for Shader {
    fn drop(&mut self) {
        let dt = &*self.device.table;
        unsafe {
            dt.destroy_shader_module(self.module, ptr::null());
        }
    }
}

impl Shader {
    crate unsafe fn new(
        device: Arc<Device>,
        code: Vec<u8>,
        stage: ShaderStage,
        // TODO: As much as possible should be drawn from reflection
        // except possibly vertex input/fragment output names.
        inputs: Vec<ShaderVar>,
        outputs: Vec<ShaderVar>,
        set_bindings: Vec<ShaderSetBinding>,
        spec_consts: Vec<SpecConstDecl>,
    ) -> Self {
        // TODO: Shader inputs, bindings, etc. could be validated via
        // reflection.
        let dt = &device.table;
        assert_eq!(code.len() % 4, 0);
        let create_info = vk::ShaderModuleCreateInfo {
            code_size: code.len(),
            p_code: code.as_ptr() as _,
            ..Default::default()
        };
        let mut module = vk::null();
        dt.create_shader_module(&create_info, ptr::null(), &mut module)
            .check().unwrap();

        let spec_consts = spec_consts.into_iter()
            .map(|decl| (decl.name, decl))
            .collect();

        Shader {
            device,
            module,
            code,
            stage,
            inputs,
            outputs,
            set_bindings,
            spec_consts,
        }
    }

    crate fn device(&self) -> &Arc<Device> {
        &self.device
    }

    crate fn module(&self) -> vk::ShaderModule {
        self.module
    }

    crate fn entry_cstr(&self) -> &CStr {
        unsafe { CStr::from_bytes_with_nul_unchecked(b"main\0") }
    }

    crate fn code(&self) -> &[u8] {
        &self.code
    }

    crate fn stage(&self) -> ShaderStage {
        self.stage
    }

    crate fn inputs(&self) -> &[ShaderVar] {
        &self.inputs
    }

    crate fn set_bindings(&self) -> &[ShaderSetBinding] {
        &self.set_bindings
    }

    crate fn spec_consts(&self) -> &FnvHashMap<Name, SpecConstDecl> {
        &self.spec_consts
    }
}

impl ShaderSpec {
    crate fn new(shader: Arc<Shader>) -> Self {
        ShaderSpec {
            shader,
            spec_info: Default::default(),
            spec_map: Default::default(),
            data: Default::default(),
        }
    }

    crate fn shader(&self) -> &Arc<Shader> {
        &self.shader
    }

    crate fn spec_info(&self) -> &vk::SpecializationInfo {
        &self.spec_info
    }

    crate fn set<T>(&mut self, name: &Name, val: &T) {
        let decl = &self.shader.spec_consts()[name];
        let bytes = std::slice::from_ref(&val).as_bytes();
        assert_eq!(bytes.len(), decl.size as usize);
        self.spec_map.push(vk::SpecializationMapEntry {
            constant_id: decl.id,
            offset: self.data.len() as u32,
            size: bytes.len(),
        });
        self.data.extend(bytes);
        self.spec_info = vk::SpecializationInfo {
            map_entry_count: self.spec_map.len() as _,
            p_map_entries: self.spec_map.as_ptr(),
            data_size: self.data.len(),
            p_data: self.data.as_ptr() as _,
        };
    }
}

impl SpecConstDecl {
    fn typed<T>(name: &str, id: u32) -> Self {
        SpecConstDecl {
            name: Name::new(name),
            id,
            size: std::mem::size_of::<T>() as _,
        }
    }
}

impl From<ShaderStage> for vk::ShaderStageFlags {
    fn from(stage: ShaderStage) -> Self {
        use ShaderStage as Stage;
        use vk::ShaderStageFlags as Flags;
        match stage {
            Stage::Vertex => Flags::VERTEX_BIT,
            Stage::TessControl => Flags::TESSELLATION_CONTROL_BIT,
            Stage::TessEval => Flags::TESSELLATION_EVALUATION_BIT,
            Stage::Geometry => Flags::GEOMETRY_BIT,
            Stage::Fragment => Flags::FRAGMENT_BIT,
            Stage::Compute => Flags::COMPUTE_BIT,
        }
    }
}

impl From<ShaderDim> for u32 {
    fn from(dim: ShaderDim) -> Self {
        match dim {
            ShaderDim::One => 1,
            ShaderDim::Two => 2,
            ShaderDim::Three => 3,
            ShaderDim::Four => 4,
        }
    }
}

impl ShaderVarType {
    crate const fn new(
        scalar: ShaderScalar,
        dim: ShaderDim,
    ) -> Self {
        Self { scalar, dim }
    }

    crate const fn int(dim: ShaderDim) -> Self {
        Self::new(ShaderScalar::Int, dim)
    }

    crate const fn uint(dim: ShaderDim) -> Self {
        Self::new(ShaderScalar::Uint, dim)
    }

    crate const fn float(dim: ShaderDim) -> Self {
        Self::new(ShaderScalar::Float, dim)
    }

    crate const fn double(dim: ShaderDim) -> Self {
        Self::new(ShaderScalar::Double, dim)
    }

    const INT: Self     = Self::int(ShaderDim::One);
    const IVEC2: Self   = Self::int(ShaderDim::Two);
    const IVEC3: Self   = Self::int(ShaderDim::Three);
    const IVEC4: Self   = Self::int(ShaderDim::Four);
    const UINT: Self    = Self::uint(ShaderDim::One);
    const UVEC2: Self   = Self::uint(ShaderDim::Two);
    const UVEC3: Self   = Self::uint(ShaderDim::Three);
    const UVEC4: Self   = Self::uint(ShaderDim::Four);
    const FLOAT: Self   = Self::float(ShaderDim::One);
    const VEC2: Self    = Self::float(ShaderDim::Two);
    const VEC3: Self    = Self::float(ShaderDim::Three);
    const VEC4: Self    = Self::float(ShaderDim::Four);
    const DOUBLE: Self  = Self::double(ShaderDim::One);
    const DVEC2: Self   = Self::double(ShaderDim::Two);
    const DVEC3: Self   = Self::double(ShaderDim::Three);
    const DVEC4: Self   = Self::double(ShaderDim::Four);
}

#[derive(Debug)]
crate struct BuiltinShaders {
    crate example_frag: Arc<Shader>,
    crate example_vert: Arc<Shader>,
}

mod sources {
    // TODO: Load from disk, hot reloading
    macro_rules! include_shaders {
        ($($ident:ident = $name:expr;)*) => {
            $(crate const $ident: &'static [u8] = include_bytes!(
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/generated/shaders/", $name, ".spv",
                )
            );)*
        }
    }

    include_shaders! {
        EXAMPLE_VERT = "example_vert";
        EXAMPLE_FRAG = "example_frag";
    }
}

macro_rules! vertex_inputs {
    ($(location($loc:expr) $type:ident $Attr:ident;)*) => {
        [$(
            ShaderVar {
                location: $loc,
                ty: ShaderVarType::$type,
                attr: Some(VertexAttrName::$Attr),
                attch: None,
            },
        )*]
    }
}

macro_rules! fragment_outputs {
    ($(location($loc:expr) $type:ident $Attch:ident;)*) => {
        [$(
            ShaderVar {
                location: $loc,
                ty: ShaderVarType::$type,
                attr: None,
                attch: Some(AttachmentName::$Attch),
            },
        )*]
    }
}

impl BuiltinShaders {
    crate fn new(
        device: &Arc<Device>,
        layouts: &BuiltinSetLayouts,
    ) -> Self {
        let binding = ShaderSetBinding::new;

        let consts = [
            SpecConstDecl::typed::<f32>("PHONG_SHININESS", 1),
        ];
        let bindings = [
            binding(0, Arc::clone(&layouts.example_globals)),
            binding(0, Arc::clone(&layouts.example_instances)),
        ];
        unsafe {
            let example_vert = Arc::new(Shader::new(
                Arc::clone(&device),
                sources::EXAMPLE_VERT.to_vec(),
                ShaderStage::Vertex,
                vertex_inputs! {
                    location(0) VEC3 Position;
                    location(1) VEC3 Normal;
                }.to_vec(),
                // Intermediates are ignored for now
                Vec::new(),
                bindings.to_vec(),
                consts.to_vec(),
            ));
            let example_frag = Arc::new(Shader::new(
                Arc::clone(&device),
                sources::EXAMPLE_FRAG.to_vec(),
                ShaderStage::Fragment,
                Vec::new(),
                fragment_outputs! {
                    location(0) VEC3 Color;
                }.to_vec(),
                bindings.to_vec(),
                consts.to_vec(),
            ));
            BuiltinShaders {
                example_frag,
                example_vert,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use enum_map::enum_map;
    use crate::*;
    use super::*;

    fn smoke_test(vars: testing::TestVars) {
        let device = Arc::clone(&vars.swapchain.device());
        let layouts = BuiltinSetLayouts::new(&device);
        let _shaders = BuiltinShaders::new(&device, &layouts);
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
