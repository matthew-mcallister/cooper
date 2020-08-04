use crate::*;

#[derive(Debug)]
pub struct RenderWorld {
    crate rloop: Box<RenderLoop>,
    crate data: RenderWorldData,
}

#[derive(Debug, Default)]
crate struct RenderWorldData {
    crate objects: Vec<RenderObject>,
    crate view: SceneView,
}

impl RenderWorld {
    pub fn new(rloop: Box<RenderLoop>) -> Self {
        Self {
            rloop,
            data: Default::default(),
        }
    }

    pub fn render_loop(&self) -> &RenderLoop {
        &self.rloop
    }

    pub fn into_inner(self) -> Box<RenderLoop> {
        self.rloop
    }

    crate fn state(&self) -> &SystemState {
        self.rloop.state()
    }

    crate fn globals(&self) -> &Globals {
        self.rloop.globals()
    }

    pub fn view(&self) -> &SceneView {
        &self.data.view
    }

    pub fn set_view(&mut self, view: SceneView) {
        self.data.view = view
    }

    pub fn frame_num(&self) -> u64 {
        self.rloop.frame_num()
    }

    pub fn add_object(&mut self, obj: impl Into<RenderObject>) {
        self.data.objects.push(obj.into());
    }

    pub fn render(self) -> Box<RenderLoop> {
        let mut rloop = self.rloop;
        let world = self.data;
        rloop.render(world);
        rloop
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use super::*;

    unsafe fn render_nothing(vars: crate::testing::TestVars) {
        let window = Arc::clone(&vars.swapchain.window());
        let app_info = (*vars.device().instance().app_info()).clone();
        let rl = Box::new(RenderLoop::new(app_info, window).unwrap());
        let world = RenderWorld::new(rl);
        world.render();
    }

    unit::declare_tests![render_nothing];
}

unit::collect_tests![tests];
