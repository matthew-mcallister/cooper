use crate::*;

#[derive(Debug)]
pub struct RenderWorld {
    crate state: Option<Box<SystemState>>,
    crate debug: Vec<DebugMesh>,
    crate view: SceneView,
}

impl RenderWorld {
    pub fn new(rloop: &mut RenderLoop) -> Self {
        let state = rloop.state.take().unwrap();
        Self {
            state: Some(state),
            debug: Vec::new(),
            view: Default::default(),
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
