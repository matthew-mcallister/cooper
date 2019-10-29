use std::ffi::{CStr, CString};
use std::ptr;

use ccore::name::*;

use crate::*;

#[derive(Clone, Debug)]
crate struct ShaderDesc {
    crate entry: CString,
    crate code: Vec<u8>,
    crate set_bindings: Vec<(u32, Name)>,
}

#[derive(Debug)]
crate struct Shader {
    inner: vk::ShaderModule,
    desc: ShaderDesc,
}

impl Shader {
    crate fn inner(&self) -> vk::ShaderModule {
        self.inner
    }

    crate fn entry(&self) -> &CStr {
        &*self.desc.entry
    }

    crate fn code(&self) -> &[u8] {
        &self.desc.code
    }

    crate fn set_bindings(&self) -> &[(u32, Name)] {
        &self.desc.set_bindings
    }
}

crate unsafe fn create_shader(
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
