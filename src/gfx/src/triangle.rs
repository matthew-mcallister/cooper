use std::sync::Arc;

use ccore::name::*;

use crate::*;

#[derive(Debug)]
crate struct TriangleTaskResult {
    crate cmds: SubpassCmds,
}

crate unsafe fn render_triangle(locals: &mut FrameLocals) -> SubpassCmds {
    let pass = Name::new("forward");
    let subpass = Name::new("lighting");

    let mut cmds = locals.create_subpass_cmds(pass, subpass);
    cmds.begin();

    let pipe = PipelineDesc {};
    cmds.bind_graphics_pipeline(&pipe);
    cmds.draw(3, 1, 0, 0);

    cmds.end();

    cmds
}

crate unsafe fn triangle_task(frame_info: Arc<FrameInfo>) -> TriangleTaskResult
{
    let cmds = frame_info.with_frame_locals(|locals| {
        render_triangle(locals)
    });
    TriangleTaskResult { cmds }
}
