use std::convert::{TryFrom, TryInto};
use std::ffi::CStr;
use std::ptr;
use std::sync::Arc;

use derivative::Derivative;
use enum_map::Enum;
use prelude::*;
use spirv_headers as spv;
use spirv_reflect::types::ReflectInterfaceVariable;

use crate::*;

#[derive(Debug)]
crate struct Shader {
    device: Arc<Device>,
    module: vk::ShaderModule,
    code: Vec<u32>,
    stage: ShaderStage,
    source_file: String,
    inputs: Vec<ShaderLocation>,
    outputs: Vec<ShaderLocation>,
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

crate type ShaderLocation = u32;

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
    crate unsafe fn new(device: Arc<Device>, code: Vec<u32>) -> Self {
        let dt = &device.table;
        let create_info = vk::ShaderModuleCreateInfo {
            code_size: 4 * code.len(),
            p_code: code.as_ptr(),
            ..Default::default()
        };
        let mut module = vk::null();
        dt.create_shader_module(&create_info, ptr::null(), &mut module)
            .check().unwrap();

        let data = code.as_bytes();
        let reflected = spirv_reflect::ShaderModule::load_u8_data(data)
            .unwrap();
        let stage = reflected.get_spirv_execution_model().try_into().unwrap();
        let (inputs, outputs) = get_shader_interface(&reflected);

        let source_file = reflected.get_source_file();
        device.set_name(module, source_file.clone());

        Shader {
            device,
            module,
            code,
            stage,
            source_file,
            inputs,
            outputs,
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

    crate fn code(&self) -> &[u32] {
        &self.code
    }

    crate fn stage(&self) -> ShaderStage {
        self.stage
    }

    crate fn inputs(&self) -> &[ShaderLocation] {
        &self.inputs
    }

    crate fn outputs(&self) -> &[ShaderLocation] {
        &self.outputs
    }

    crate fn name(&self) -> Option<&str> {
        Some(&self.source_file)
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

    crate fn set<T>(&mut self, id: u32, val: &T) -> &mut ShaderSpec {
        let bytes = std::slice::from_ref(val).as_bytes();
        self.spec_map.push(vk::SpecializationMapEntry {
            constant_id: id,
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

fn get_shader_interface(reflected: &spirv_reflect::ShaderModule) ->
    (Vec<ShaderLocation>, Vec<ShaderLocation>)
{
    let vars = |vars: Vec<ReflectInterfaceVariable>| {
        let mut vars: Vec<_> = vars.into_iter()
            .filter_map(|var| (var.location != !0).then_some(var.location))
            .collect();
        vars.sort_unstable();
        vars
    };
    let inputs = vars(reflected.enumerate_input_variables(None).unwrap());
    let outputs = vars(reflected.enumerate_output_variables(None).unwrap());
    (inputs, outputs)
}

impl From<Arc<Shader>> for ShaderSpec {
    fn from(shader: Arc<Shader>) -> Self {
        Self::new(shader)
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

impl TryFrom<spv::ExecutionModel> for ShaderStage {
    type Error = ();
    fn try_from(val: spv::ExecutionModel) -> Result<Self, Self::Error> {
        Ok(match val {
            spv::ExecutionModel::Vertex => Self::Vertex,
            spv::ExecutionModel::TessellationControl => Self::TessControl,
            spv::ExecutionModel::TessellationEvaluation => Self::TessEval,
            spv::ExecutionModel::Geometry => Self::Geometry,
            spv::ExecutionModel::Fragment => Self::Fragment,
            spv::ExecutionModel::GLCompute => Self::Compute,
            _ => return Err(()),
        })
    }
}
