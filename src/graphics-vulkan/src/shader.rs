use std::ffi::{CStr, CString};
use std::ptr;
use std::sync::Arc;

use fnv::FnvHashMap;
use prelude::*;

use crate::*;

#[cfg(test)]
macro_rules! include_shader {
    ($name:expr) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            concat!("/generated/shaders/", $name),
        ))
    }
}

#[derive(Debug)]
pub struct ShaderDesc {
    pub entry: CString,
    pub code: Vec<u8>,
    pub set_bindings: Vec<(u32, String)>,
}

#[cfg(test)]
macro_rules! create_shader {
    (
        $shaders:expr,
        {
            name: $name:expr,
            bindings: [$(($binding_idx:expr, $binding_name:expr)),*$(,)*]$(,)*
        }$(,)*
    ) => {
        let desc = ShaderDesc {
            entry: CString::new("main".to_owned()).unwrap(),
            code: include_shader!(concat!($name, ".spv")).to_vec(),
            set_bindings: vec![$(($binding_idx, $binding_name.to_owned()),)*],
        };
        $shaders.create_shader($name.to_owned(), desc);
    }
}

#[derive(Debug)]
pub struct Shader {
    pub inner: vk::ShaderModule,
    pub desc: ShaderDesc,
}

impl Shader {
    pub fn entry(&self) -> &CStr {
        &*self.desc.entry
    }
}

#[derive(Debug)]
pub struct ShaderManager {
    crate device: Arc<Device>,
    shaders: FnvHashMap<String, Shader>,
}

impl Drop for ShaderManager {
    fn drop(&mut self) {
        let dt = &self.device.table;
        unsafe {
            for shader in self.shaders.values() {
                dt.destroy_shader_module(shader.inner, ptr::null());
            }
        }
    }
}

impl ShaderManager {
    pub fn new(device: Arc<Device>) -> Self {
        ShaderManager {
            device,
            shaders: Default::default(),
        }
    }

    pub unsafe fn create_shader(&mut self, name: String, desc: ShaderDesc) {
        let dt = &self.device.table;

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

        let shader = Shader {
            inner,
            desc,
        };

        insert_unique!(self.shaders, name, shader);
    }

    pub fn get(&self, key: impl AsRef<str>) -> &Shader {
        &self.shaders[key.as_ref()]
    }

    pub fn len(&self) -> usize {
        self.shaders.len()
    }
}

#[cfg(test)]
crate unsafe fn create_test_shaders(vars: &testing::TestVars) ->
    Arc<ShaderManager>
{
    let device = Arc::clone(&vars.swapchain.device);

    let mut shader_man = ShaderManager::new(device);
    create_shader!(shader_man, {
        name: "example_vert",
        bindings: [(0, "scene_globals")],
    });
    create_shader!(shader_man, {
        name: "example_frag",
        bindings: [(0, "scene_globals")],
    });

    Arc::new(shader_man)
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let shaders = create_test_shaders(&vars);
        assert_ne!(shaders.shaders.len(), 0);
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
