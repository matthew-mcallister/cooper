use std::sync::Arc;

use crate::{SystemState, Globals};
use crate::device::{DescriptorSet, ShaderStageMap};
use crate::resource::ResourceSystem;
use super::*;

#[allow(unused_variables)]
pub(super) trait MaterialFactory: std::fmt::Debug + Send + Sync {
    fn process_image_bindings(&self, images: &mut MaterialImageBindings) {}

    fn create_descriptor_set(
        &self,
        state: &SystemState,
        images: &MaterialImageState,
    ) -> Option<DescriptorSet> {
        None
    }

    // TODO: Not necessary.
    fn select_shaders(&self) -> ShaderStageMap;
}

#[derive(Debug)]
crate struct MaterialSystem {
    factories: EnumMap<MaterialProgram, Arc<dyn MaterialFactory>>,
    materials: MaterialStateTable,
}

impl MaterialSystem {
    crate fn new(state: &SystemState, globals: &Arc<Globals>) -> Self {
        let factories = EnumMap::from(|prog| match prog {
            MaterialProgram::Checker => Arc::new(SimpleMaterialFactory::new(
                state, globals, SimpleMode::Checker,
            )),
            MaterialProgram::FragDepth => Arc::new(SimpleMaterialFactory::new(
                state, globals, SimpleMode::Depth,
            )),
            MaterialProgram::FragNormal => Arc::new(SimpleMaterialFactory::new(
                state, globals, SimpleMode::Normal,
            )),
            MaterialProgram::Albedo =>
                Arc::new(TextureVisMaterialFactory::new(
                    state, globals, MaterialImage::Albedo,
                )),
            MaterialProgram::NormalMap =>
                Arc::new(TextureVisMaterialFactory::new(
                    state, globals, MaterialImage::Normal,
                )),
            MaterialProgram::MetallicRoughness =>
                Arc::new(TextureVisMaterialFactory::new(
                    state, globals, MaterialImage::MetallicRoughness,
                )),
        }: Arc<dyn MaterialFactory>);
        Self {
            factories,
            materials: MaterialStateTable::new(),
        }
    }

    crate fn define_material(
        &self,
        program: MaterialProgram,
        mut images: MaterialImageBindings,
    ) -> Arc<MaterialDef> {
        let factory = Arc::clone(&self.factories[program]);
        factory.process_image_bindings(&mut images);
        Arc::new(MaterialDef {
            factory,
            program,
            image_bindings: images,
        })
    }

    crate fn get_or_create(
        &mut self,
        state: &SystemState,
        resources: &ResourceSystem,
        def: &Arc<MaterialDef>,
    ) -> Result<&Arc<Material>, ResourceUnavailable> {
        self.materials.get_or_create(state, resources, def)
    }
}
