use std::ffi::CStr;
use std::ptr;
use std::sync::Arc;

use base::Name;
use derivative::Derivative;
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
    spec_consts: FnvHashMap<Name, SpecConstDecl>,
}

/// A choice of shader plus specialization constant values.
#[derive(Debug, Derivative)]
#[derivative(Hash, PartialEq)]
crate struct ShaderSpec {
    #[derivative(Hash(hash_with="ptr_hash"))]
    #[derivative(PartialEq(compare_with="ptr_eq"))]
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

/// The GLSL type of a shader interface (`in` or `out`) variable.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
crate struct ShaderVarType {
    crate scalar: ShaderScalar,
    crate dim: Dimension,
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
        spec_consts: Vec<SpecConstDecl>,
    ) -> Self {
        // TODO: Shader inputs, etc. could be validated via reflection.
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

    crate fn set<T>(&mut self, name: &Name, val: &T) -> &mut ShaderSpec {
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
        self
    }
}

impl From<Arc<Shader>> for ShaderSpec {
    fn from(shader: Arc<Shader>) -> Self {
        Self::new(shader)
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

impl ShaderVarType {
    crate const fn new(
        scalar: ShaderScalar,
        dim: Dimension,
    ) -> Self {
        Self { scalar, dim }
    }

    crate const fn int(dim: Dimension) -> Self {
        Self::new(ShaderScalar::Int, dim)
    }

    crate const fn uint(dim: Dimension) -> Self {
        Self::new(ShaderScalar::Uint, dim)
    }

    crate const fn float(dim: Dimension) -> Self {
        Self::new(ShaderScalar::Float, dim)
    }

    crate const fn double(dim: Dimension) -> Self {
        Self::new(ShaderScalar::Double, dim)
    }

    crate const INT: Self     = Self::int(Dimension::One);
    crate const IVEC2: Self   = Self::int(Dimension::Two);
    crate const IVEC3: Self   = Self::int(Dimension::Three);
    crate const IVEC4: Self   = Self::int(Dimension::Four);
    crate const UINT: Self    = Self::uint(Dimension::One);
    crate const UVEC2: Self   = Self::uint(Dimension::Two);
    crate const UVEC3: Self   = Self::uint(Dimension::Three);
    crate const UVEC4: Self   = Self::uint(Dimension::Four);
    crate const FLOAT: Self   = Self::float(Dimension::One);
    crate const VEC2: Self    = Self::float(Dimension::Two);
    crate const VEC3: Self    = Self::float(Dimension::Three);
    crate const VEC4: Self    = Self::float(Dimension::Four);
    crate const DOUBLE: Self  = Self::double(Dimension::One);
    crate const DVEC2: Self   = Self::double(Dimension::Two);
    crate const DVEC3: Self   = Self::double(Dimension::Three);
    crate const DVEC4: Self   = Self::double(Dimension::Four);
}
