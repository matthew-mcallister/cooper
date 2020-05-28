use std::convert::TryFrom;
use std::sync::Arc;

use enum_map::EnumMap;

use crate::*;

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
#[non_exhaustive]
enum SimpleMode {
    Checker = 0,
    Depth = 1,
    Normal = 2,
}

impl TryFrom<MaterialProgram> for SimpleMode {
    type Error = &'static str;
    #[allow(unreachable_patterns)]
    fn try_from(prog: MaterialProgram) -> Result<Self, Self::Error> {
        Ok(match prog {
            MaterialProgram::Checker => SimpleMode::Checker,
            MaterialProgram::FragDepth => SimpleMode::Depth,
            MaterialProgram::FragNormal => SimpleMode::Normal,
            _ => return Err("invalid SimpleMode"),
        })
    }
}

#[derive(Debug)]
crate struct SimpleMaterialFactory {
    mode: SimpleMode,
    vert_shader: Arc<ShaderSpec>,
    frag_shader: Arc<ShaderSpec>,
}

impl SimpleMaterialFactory {
    crate fn new(_state: &SystemState, globals: &Arc<Globals>) -> [Self; 3] {
        let vert_shader =
            Arc::new(Arc::clone(&globals.shaders.static_vert).into());
        let mk_rend = |mode| {
            let shader = Arc::clone(&globals.shaders.simple_frag);
            let mut spec = ShaderSpec::new(shader);
            spec.set(ShaderConst::SimpleMode as _, &(mode as u32));
            Self {
                mode,
                vert_shader: Arc::clone(&vert_shader),
                frag_shader: Arc::new(spec),
            }
        };

        [
            mk_rend(SimpleMode::Checker),
            mk_rend(SimpleMode::Depth),
            mk_rend(SimpleMode::Normal),
        ]
    }
}

impl MaterialFactory for SimpleMaterialFactory {
    fn create_descriptor_set(
        &self,
        _images: &MaterialImageMap,
    ) -> Option<DescriptorSet> {
        None
    }

    fn select_shaders(&self, skinned: bool) -> ShaderStageMap {
        let mut map = EnumMap::default();
        assert!(!skinned);
        map[ShaderStage::Vertex] = Some(Arc::clone(&self.vert_shader));
        map[ShaderStage::Fragment] = Some(Arc::clone(&self.frag_shader));
        map
    }
}
