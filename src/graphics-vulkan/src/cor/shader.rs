use std::ffi::{CStr, CString};
use std::ptr;

use crate::*;

#[derive(Clone, Debug)]
pub struct ShaderDesc {
    pub entry: CString,
    pub code: Vec<u8>,
    pub set_bindings: Vec<(u32, String)>,
}

#[derive(Debug)]
pub struct Shader {
    inner: vk::ShaderModule,
    desc: ShaderDesc,
}

impl Shader {
    pub fn inner(&self) -> vk::ShaderModule {
        self.inner
    }

    pub fn entry(&self) -> &CStr {
        &*self.desc.entry
    }

    pub fn code(&self) -> &[u8] {
        &self.desc.code
    }

    pub fn set_bindings(&self) -> &[(u32, String)] {
        &self.desc.set_bindings
    }
}

pub unsafe fn create_shader(
    device: &Device,
    desc: ShaderDesc,
) -> Shader {
    let dt = &device.table;
    let code = &desc.code;
    assert_eq!(code.len() % 4, 0);
    let create_info = vk::ShaderModuleCreateInfo {
        code_size: code.len(),
        p_code: code.as_ptr() as _,
        ..Default::default()
    };
    let mut inner = vk::null();
    dt.create_shader_module(&create_info, ptr::null(), &mut inner)
        .check().unwrap();
    Shader { inner, desc }
}
