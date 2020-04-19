use crate::*;

#[derive(Debug)]
pub struct RenderWorld {
    crate state: Option<Box<SystemState>>,
    crate debug: Vec<DebugMesh>,
    crate view: SceneView,
    crate frame_num: u64,
}

impl RenderWorld {
    pub fn new(rloop: &mut RenderLoop) -> Self {
        let frame_num = rloop.frame_num();
        let state = rloop.state.take().unwrap();
        Self {
            state: Some(state),
            debug: Vec::new(),
            view: Default::default(),
            frame_num,
        }
    }

    crate fn state(&self) -> &SystemState {
        self.state.as_ref().unwrap()
    }

    pub fn add_debug(&mut self, mesh: DebugMesh) {
        self.debug.push(mesh)
    }

    pub fn view(&self) -> &SceneView {
        &self.view
    }

    pub fn set_view(&mut self, view: SceneView) {
        self.view = view
    }

    pub fn frame_num(&self) -> u64 {
        self.frame_num
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use super::*;

    unsafe fn smoke_test(vars: crate::testing::TestVars) {
        let window = Arc::clone(&vars.swapchain.surface.window);
        let app_info = (*vars.device().instance.app_info).clone();
        let mut rl = RenderLoop::new(app_info, window).unwrap();
        let world = RenderWorld::new(&mut rl);
        rl.render(world);
    }

    unit::declare_tests![smoke_test];
}

unit::collect_tests![tests];
