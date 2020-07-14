use std::convert::TryFrom;
use std::sync::Arc;

use base::partial_map;

use crate::*;
use super::*;

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
#[non_exhaustive]
pub(super) enum SimpleMode {
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
pub(super) struct SimpleMaterialFactory {
    mode: SimpleMode,
    vert_shader: Arc<ShaderSpec>,
    frag_shader: Arc<ShaderSpec>,
}

impl SimpleMaterialFactory {
    pub(super) fn new(
        _state: &SystemState,
        globals: &Arc<Globals>,
        mode: SimpleMode,
    ) -> Self {
        let vert_shader =
            Arc::new(Arc::clone(&globals.shaders.static_vert).into());

        let shader = Arc::clone(&globals.shaders.simple_frag);
        let mut spec = ShaderSpec::new(shader);
        spec.set(ShaderConst::SimpleMode as _, &(mode as u32));

        Self {
            mode,
            vert_shader,
            frag_shader: Arc::new(spec),
        }
    }
}

impl MaterialFactory for SimpleMaterialFactory {
    fn select_shaders(&self) -> ShaderStageMap {
        partial_map! {
            ShaderStage::Vertex => Arc::clone(&self.vert_shader),
            ShaderStage::Fragment => Arc::clone(&self.frag_shader),
        }
    }
}
