use std::convert::{TryFrom, TryInto};
use std::ffi::CStr;
use std::ptr;
use std::sync::Arc;

use enum_map::Enum;

use crate::*;

#[derive(Debug)]
pub struct Shader {
    device: Arc<Device>,
    inner: vk::ShaderModule,
    code: Vec<u32>,
    stage: ShaderStage,
    source_file: Option<String>,
    inputs: Vec<ShaderLocation>,
    outputs: Vec<ShaderLocation>,
    // TODO: reflect uniforms so we can make sure all descriptors are
    // bound.
}

/// A choice of shader plus specialization constant values.
// TODO: How to properly hash this? Especially considering all consts
// have defaults defined in the shader.
#[derive(Debug, Eq, Hash, PartialEq)]
pub struct ShaderSpec {
    shader: Arc<Shader>,
    spec_info: vk::SpecializationInfo,
    spec_map: Vec<vk::SpecializationMapEntry>,
    data: Vec<u8>,
}

// Probably should know the type and dimension too...
pub type ShaderLocation = u32;

#[derive(Clone, Copy, Debug, Enum, Eq, Hash, PartialEq)]
pub enum ShaderStage {
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
            dt.destroy_shader_module(self.inner, ptr::null());
        }
    }
}

impl_device_derived!(Shader);

impl Shader {
    pub unsafe fn new(device: Arc<Device>, code: Vec<u32>, source_file: Option<String>) -> Self {
        let dt = &device.table;
        let create_info = vk::ShaderModuleCreateInfo {
            code_size: 4 * code.len(),
            p_code: code.as_ptr(),
            ..Default::default()
        };
        let mut inner = vk::null();
        dt.create_shader_module(&create_info, ptr::null(), &mut inner)
            .check()
            .unwrap();

        let reflected = spv::parse_words(&code);
        let entry = reflected.get_entry_point(&"main").unwrap();
        let stage = entry.execution_model().try_into().unwrap();
        let (inputs, outputs) = get_shader_interface(&entry);

        if let Some(source) = &source_file {
            device.set_name(inner, source.clone());
        }

        Shader {
            device,
            inner,
            code,
            stage,
            source_file,
            inputs,
            outputs,
        }
    }

    /// A convenience method for loading a shader off the disk.
    pub unsafe fn from_path(device: Arc<Device>, path: impl Into<String>) -> std::io::Result<Self> {
        use byteorder::{ByteOrder, NativeEndian};
        let path = path.into();
        let bytes = std::fs::read(&path)?;
        if bytes.len() % 4 != 0 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        let mut words = Vec::with_capacity(bytes.len() / 4);
        words.set_len(words.capacity());
        NativeEndian::read_u32_into(&bytes, &mut words);
        Ok(Self::new(device, words, Some(path)))
    }

    #[inline]
    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    #[inline]
    pub fn module(&self) -> vk::ShaderModule {
        self.inner
    }

    #[inline]
    pub fn entry_cstr(&self) -> &CStr {
        unsafe { CStr::from_bytes_with_nul_unchecked(b"main\0") }
    }

    #[inline]
    pub fn code(&self) -> &[u32] {
        &self.code
    }

    #[inline]
    pub fn stage(&self) -> ShaderStage {
        self.stage
    }

    #[inline]
    pub fn inputs(&self) -> &[ShaderLocation] {
        &self.inputs
    }

    #[inline]
    pub fn outputs(&self) -> &[ShaderLocation] {
        &self.outputs
    }
}

impl Named for Shader {
    fn name(&self) -> Option<&str> {
        Some(&self.source_file.as_ref()?)
    }
}

impl ShaderSpec {
    #[inline]
    pub fn new(shader: Arc<Shader>) -> Self {
        ShaderSpec {
            shader,
            spec_info: Default::default(),
            spec_map: Default::default(),
            data: Default::default(),
        }
    }

    #[inline]
    pub fn shader(&self) -> &Arc<Shader> {
        &self.shader
    }

    #[inline]
    pub fn spec_info(&self) -> &vk::SpecializationInfo {
        &self.spec_info
    }

    pub fn set<T>(&mut self, id: u32, val: &T) -> &mut ShaderSpec {
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

fn get_shader_interface(entry: &spv::EntryPoint<'_>) -> (Vec<ShaderLocation>, Vec<ShaderLocation>) {
    let mut inputs: Vec<_> = entry.inputs().map(|input| input.location()).collect();
    inputs.sort();
    let mut outputs: Vec<_> = entry.outputs().map(|output| output.location()).collect();
    outputs.sort();
    (inputs, outputs)
}

impl From<Arc<Shader>> for ShaderSpec {
    fn from(shader: Arc<Shader>) -> Self {
        Self::new(shader)
    }
}

impl From<ShaderStage> for vk::ShaderStageFlags {
    fn from(stage: ShaderStage) -> Self {
        use vk::ShaderStageFlags as Flags;
        use ShaderStage as Stage;
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
