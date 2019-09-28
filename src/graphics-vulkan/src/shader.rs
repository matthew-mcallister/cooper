use std::ptr;
use std::sync::Arc;

use ccore::node::*;
use fnv::FnvHashMap;

use crate::*;

macro_rules! include_shader {
    ($name:expr) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            concat!("/generated/shaders/", $name),
        ))
    }
}

#[derive(Debug)]
pub struct ShaderDef {
    pub name: String,
    pub code: Vec<u8>,
    pub bindings: Vec<(u32, String)>,
}

#[macro_export]
macro_rules! shader_def {
    (
        name: $name:expr,
        bindings: [$(($binding_idx:expr, $binding_name:expr)),*$(,)*]$(,)*
    ) => {
        ShaderDef {
            name: $name.to_owned(),
            code: include_shader!(concat!($name, ".spv")).to_vec(),
            bindings: vec![$(($binding_idx, $binding_name.to_owned()),)*],
        }
    }
}

pub fn get_shader_defs() -> Vec<ShaderDef> {
    vec![
        shader_def! {
            name: "cube_vert",
            bindings: [
                (0, "cube"),
            ],
        },
        shader_def! {
            name: "cube_frag",
            bindings: [
                (0, "cube"),
            ],
        },
    ]
}

#[derive(Debug)]
pub struct Shader {
    inner: vk::ShaderModule,
    def: ShaderDef,
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

    pub unsafe fn create_shader(&mut self, def: ShaderDef) -> Id<Shader> {
        let dt = &self.device.table;

        let code = &def.code;
        assert_eq!(code.len() % 4, 0);
        let create_info = vk::ShaderModuleCreateInfo {
            code_size: code.len(),
            p_code: code.as_ptr() as _,
            ..Default::default()
        };

        let mut inner = vk::null();
        dt.create_shader_module
            (&create_info as _, ptr::null(), &mut inner as _)
            .check().unwrap();

        let name = def.name.clone();

        let shader = Shader {
            inner,
            def,
        };

        let id = self.shaders.add(shader);
        self.shaders_by_name.insert(name, id);

        id
    }
}

pub unsafe fn create_shaders(device: Arc<Device>) -> Arc<ShaderManager> {
    let mut shade_man = ShaderManager::new(device);
    for def in get_shader_defs() {
        shade_man.create_shader(def);
    }
    Arc::new(shade_man)
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn smoke_test(vars: testing::TestVars) {
        let device = Arc::clone(&vars.swapchain.device);
        let shade_man = create_shaders(device);
        assert_ne!(shade_man.shaders.len(), 0);
        for (name, &id) in shade_man.shaders_by_name.iter() {
            assert_eq!(&name[..], &shade_man.shaders[id].def.name[..]);
        }
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
