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
        cmds.bind_gfx_pipe(&item.pipeline);

        // TODO: this could be done outside the loop
        if i == 0 { cmds.bind_gfx_descs(0, descriptors.inner()); }
        cmds.bind_gfx_descs(1, &item.descriptors);

        let mesh = &item.mesh;
        bind_mesh_data(cmds, mesh);
        let fst_inst = i as u32;
        let count = mesh.vertex_count();
        if mesh.index().is_some() {
            cmds.draw_indexed_offset(count, 1, 0, 0, fst_inst);
        } else {
            cmds.draw_offset(count, 1, 0, fst_inst);
        }
    }
}

// TODO: Cmd buffer extension trait?
fn bind_mesh_data(cmds: &mut SubpassCmds, mesh: &MeshData) {
    let buffers = mesh.attrs.iter().map(|attr| attr.buffer.range());
    cmds.bind_vertex_buffers(buffers);
    if let Some(index) = mesh.index() {
        cmds.bind_index_buffer(index.buffer.range(), index.ty);
    }
}
