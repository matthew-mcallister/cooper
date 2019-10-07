use std::ffi::{CStr, CString};
use std::ptr;
use std::sync::Arc;

use ccore::node::*;
use fnv::FnvHashMap;

use crate::*;

#[allow(unused_macros)]
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
    pub name: String,
    pub entry: CString,
    pub code: Vec<u8>,
    pub set_bindings: Vec<(u32, String)>,
}

#[macro_export]
macro_rules! shader_desc {
    (
        name: $name:expr,
        entry: $entry:expr,
        bindings: [$(($binding_idx:expr, $binding_name:expr)),*$(,)*]$(,)*
    ) => {
        ShaderDesc {
            name: $name.to_owned(),
            entry: CString::new($entry.to_owned()).unwrap(),
            code: include_shader!(concat!($name, ".spv")).to_vec(),
            set_bindings: vec![$(($binding_idx, $binding_name.to_owned()),)*],
        }
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
    pub device: Arc<Device>,
    pub shaders: NodeArray<Shader>,
    pub shaders_by_name: FnvHashMap<String, Id<Shader>>,
}

impl Drop for ShaderManager {
    fn drop(&mut self) {
        let dt = &self.device.table;
        unsafe {
            for (_, shader) in self.shaders.iter() {
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
            shaders_by_name: Default::default(),
        }
    }

    pub unsafe fn create_shader(&mut self, desc: ShaderDesc) -> Id<Shader> {
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

        let name = desc.name.clone();

        let shader = Shader {
            inner,
            desc,
        };

        let id = self.shaders.add(shader);
        self.shaders_by_name.insert(name, id);

        id
    }

    pub fn by_name(&self, name: impl AsRef<str>) -> &Shader {
        let name = name.as_ref();
        let id = self.shaders_by_name[name];
        &self.shaders[id]
    }
}

#[cfg(test)]
crate unsafe fn create_test_shaders(vars: &testing::TestVars) ->
    Arc<ShaderManager>
{
    let device = Arc::clone(&vars.swapchain.device);

    let descs = vec![
        shader_desc! {
            name: "cube_vert",
            entry: "main",
            bindings: [
                (0, "scene_globals"),
            ],
        },
        shader_desc! {
            name: "cube_frag",
            entry: "main",
            bindings: [
                (1, "material"),
            ],
        },
    ];

    let mut shader_man = ShaderManager::new(device);
    for desc in descs {
        shader_man.create_shader(desc);
    }
    Arc::new(shader_man)
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let shade_man = create_test_shaders(&vars);
        assert_ne!(shade_man.shaders.len(), 0);
        for (name, &id) in shade_man.shaders_by_name.iter() {
            assert_eq!(&name[..], &shade_man.shaders[id].desc.name[..]);
        }
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
