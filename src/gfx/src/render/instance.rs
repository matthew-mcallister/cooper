use std::sync::Arc;

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
        view: &SceneViewState,
        descriptors: &SceneDescriptors,
        instances: &[RenderItem],
        cmds: &mut SubpassCmds,
    ) {
        unsafe {
            render_instances(&self.state, view, descriptors, instances, cmds);
        }
    }
}

unsafe fn render_instances(
    state: &SystemState,
    view: &SceneViewState,
    descriptors: &SceneDescriptors,
    instances: &[RenderItem],
    cmds: &mut SubpassCmds,
) {
    if instances.is_empty() { return; }

    let mut desc = GraphicsPipelineDesc::new(cmds.subpass().clone());
    desc.cull_mode = view.force_cull_mode
        .unwrap_or(CullMode::Back);
    desc.depth_test = true;
    desc.depth_write = true;
    desc.depth_cmp_op = vk::CompareOp::GREATER;

    for (i, instance) in instances.iter().enumerate() {
        let mesh = &instance.mesh;
        let material = &instance.material;

        desc.stages = material.select_shaders();

        let set_layouts = &mut desc.layout.set_layouts;
        *set_layouts = vec![Arc::clone(descriptors.layout())];
        tryopt! {
            set_layouts.push(Arc::clone(material.desc()?.layout()));
        };

        let vertex_shader = desc.vertex_stage().shader();
        desc.vertex_layout = mesh.vertex_layout()
            .input_layout_for_shader(&vertex_shader);

        let pipeline = state.pipelines.get_or_create_gfx(&desc);
        cmds.bind_gfx_pipe(&pipeline);

        if i == 0 {
            // TODO: this could be done outside the loop
            cmds.bind_gfx_descs(0, descriptors.inner());
        }
        tryopt! {
            cmds.bind_gfx_descs(1, material.desc()?);
        };

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
