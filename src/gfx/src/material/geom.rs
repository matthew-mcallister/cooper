use std::convert::TryFrom;
use std::sync::Arc;

use base::partial_map;

use crate::*;
use super::*;

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
#[non_exhaustive]
pub(super) enum GeomVisMode {
    Checker = 0,
    Depth = 1,
    Normal = 2,
}

impl TryFrom<MaterialProgram> for GeomVisMode {
    type Error = &'static str;
    #[allow(unreachable_patterns)]
    fn try_from(prog: MaterialProgram) -> Result<Self, Self::Error> {
        Ok(match prog {
            MaterialProgram::Checker => GeomVisMode::Checker,
            MaterialProgram::GeomDepth => GeomVisMode::Depth,
            MaterialProgram::GeomNormal => GeomVisMode::Normal,
            _ => return Err("invalid GeomVisMode"),
        })
    }
}

#[derive(Debug)]
pub(super) struct GeomVisMaterialFactory {
    mode: GeomVisMode,
    vert_shader: Arc<ShaderSpec>,
    frag_shader: Arc<ShaderSpec>,
}

impl GeomVisMaterialFactory {
    pub(super) fn new(
        _state: &SystemState,
        globals: &Arc<Globals>,
        mode: GeomVisMode,
    ) -> Self {
        let vert_shader =
            Arc::new(Arc::clone(&globals.shaders.static_vert).into());

        let shader = Arc::clone(&globals.shaders.geom_vis_frag);
        let mut spec = ShaderSpec::new(shader);
        spec.set(ShaderConst::GeomVisMode as _, &(mode as u32));

        Self {
            mode,
            vert_shader,
            frag_shader: Arc::new(spec),
        }
    }
}

impl MaterialFactory for GeomVisMaterialFactory {
    fn select_shaders(&self) -> ShaderStageMap {
        partial_map! {
            ShaderStage::Vertex => Arc::clone(&self.vert_shader),
            ShaderStage::Fragment => Arc::clone(&self.frag_shader),
        }
    }
}
