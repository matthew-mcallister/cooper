use std::sync::Arc;

use crate::{SystemState, Globals};
use crate::device::{DescriptorSet, ShaderStageMap};
use crate::resource::ResourceSystem;
use super::*;

pub(super) trait MaterialFactory: std::fmt::Debug + Send + Sync {
    fn create_descriptor_set(
        &self,
        state: &SystemState,
        globals: &Globals,
        images: &MaterialImageState,
    ) -> Option<DescriptorSet>;

    // TODO: Not necessary.
    fn select_shaders(&self) -> ShaderStageMap;
}

#[derive(Debug)]
crate struct MaterialSystem {
    factories: EnumMap<MaterialProgram, Arc<dyn MaterialFactory>>,
    materials: MaterialStateTable,
}

impl MaterialSystem {
    crate fn new(_state: &SystemState, globals: &Arc<Globals>) -> Self {
        let [checker, depth, normal] =
            SimpleMaterialFactory::new(_state, globals);
        let factories = unsafe { std::mem::transmute([
             Arc::new(checker),  // Checker
             Arc::new(depth),    // FragDepth
             Arc::new(normal),   // FragNormal
        ]: [Arc<dyn MaterialFactory>; 3]) };
        Self {
            factories,
            materials: MaterialStateTable::new(),
        }
    }

    crate fn define_material(
        &self,
        program: MaterialProgram,
        images: MaterialImageBindings,
    ) -> Arc<MaterialDef> {
        let factory = Arc::clone(&self.factories[program]);
        Arc::new(MaterialDef {
            factory,
            program,
            image_bindings: images,
        })
    }

    crate fn get_or_create(
        &mut self,
        state: &SystemState,
        globals: &Globals,
        resources: &ResourceSystem,
        def: &Arc<MaterialDef>,
    ) -> Result<&Arc<Material>, ResourceUnavailable> {
        self.materials.get_or_create(state, globals, resources, def)
    }
}
