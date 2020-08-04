use std::sync::Arc;

use device::*;

use crate::*;

#[derive(Debug)]
#[non_exhaustive]
crate struct InstanceRenderer {
    state: Arc<Box<SystemState>>,
}

impl InstanceRenderer {
    crate fn new(state: &Arc<Box<SystemState>>, _globals: &Arc<Globals>) ->
        Self
    {
        InstanceRenderer {
            state: Arc::clone(state),
        }
    }

    crate fn render(
        &mut self,
        descriptors: &SceneDescriptors,
        instances: &[RenderItem],
        cmds: &mut SubpassCmds,
    ) {
        unsafe {
            render_instances(descriptors, instances, cmds);
        }
    }
}

unsafe fn render_instances(
    descriptors: &SceneDescriptors,
    instances: &[RenderItem],
    cmds: &mut SubpassCmds,
) {
    if instances.is_empty() { return; }
    for (i, item) in instances.iter().enumerate() {
        let mesh = &item.mesh;

        cmds.bind_gfx_pipe(&item.pipeline);

        // TODO: this could be done outside the loop
        if i == 0 { cmds.bind_gfx_descs(0, descriptors.inner()); }
        cmds.bind_gfx_descs(1, &item.descriptors);

        let fst_inst = i as u32;
        cmds.bind_vertex_buffers(&mesh.data());
        if let Some(index) = mesh.index() {
            let vert_count = index.count();
            cmds.bind_index_buffer(index.data(), index.ty());
            cmds.draw_indexed_offset(vert_count, 1, 0, 0, fst_inst);
        } else {
            let vert_count = mesh.vertex_count();
            cmds.draw_offset(vert_count, 1, 0, fst_inst);
        }
    }
}
