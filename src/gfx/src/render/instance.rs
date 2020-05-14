use std::mem::MaybeUninit;
use std::sync::Arc;

use crate::*;

// TODO: When should pipelines be compiled? When the instance is placed
// in the queue? As a batch job? Or is the current implementation fine?
#[derive(Clone, Debug)]
pub struct MeshInstance {
    pub mesh: Arc<RenderMesh>,
    pub material: Arc<Material>,
    /// Assumed to be orthogonal.
    pub rot: [[f32; 3]; 3],
    pub pos: [f32; 3],
    //TODO:
    //pub scale: f32,
}

#[derive(Clone, Copy, Debug)]
#[repr(C, align(16))]
struct PerInstanceData {
    xform: [[f32; 4]; 3],
}

#[derive(Debug)]
crate struct InstanceRenderer {
    instance_desc: DescriptorSet,
}

impl InstanceRenderer {
    crate fn new(state: &SystemState, globals: &Arc<Globals>) -> Self {
        let desc = state.descriptors.alloc(&globals.instance_buf_layout);
        state.device.set_name(&desc, "instance_desc");
        InstanceRenderer {
            instance_desc: desc,
        }
    }

    crate fn render(
        &mut self,
        view: &SceneViewState,
        instances: Vec<MeshInstance>,
        cmds: &mut SubpassCmds,
    ) {
        unsafe {
            render_instances(&mut self.instance_desc, view, instances, cmds);
        }
    }
}

// TODO: Sort meshes by pipeline, or at least display type
// TODO: Instance sorted meshes.
// TODO: In the case of vertex shader skinning, we need an extra
// per-instance binding (the joints) and a different vertex shader.
// This can be accommodated; also, compute skinning is a workaround.
unsafe fn render_instances(
    instance_desc: &mut DescriptorSet,
    view: &SceneViewState,
    instances: Vec<MeshInstance>,
    cmds: &mut SubpassCmds,
) {
    if instances.is_empty() { return; }

    let state = view.state();
    let globals = view.globals();

    let mut instance_data = view.state().buffers.box_uninit(
        BufferBinding::Storage,
        Lifetime::Frame,
        instances.len(),
    );
    // TODO: Upload to device-local memory?
    // TODO: Use dynamic offset buffers to skip this?
    instance_desc.write_buffer(0, instance_data.range());

    let mut desc = GraphicsPipelineDesc::new(
        cmds.subpass().clone(),
        Arc::clone(&globals.empty_pipeline_layout),
    );
    desc.cull_mode = view.force_cull_mode
        .unwrap_or(vk::CullModeFlags::BACK_BIT);
    desc.depth_test = true;
    desc.depth_write = true;
    desc.depth_cmp_op = vk::CompareOp::GREATER;

    for (i, instance) in instances.into_iter().enumerate() {
        let mesh = instance.mesh;
        let material = instance.material;

        let m = affine_xform(instance.rot, instance.pos);
        let mv = mat_x_mat(view.uniforms.view, m);
        instance_data[i] = MaybeUninit::new(PerInstanceData {
            xform: pack_affine_xform(mv)
        });

        desc.layout = Arc::clone(material.pipeline_layout());
        desc.stages = material.select_shaders(false);

        let vertex_shader = desc.stages[ShaderStage::Vertex]
            .as_ref().unwrap().shader();
        desc.vertex_layout =
            VertexInputLayout::new(&mesh.vertex_layout(), &vertex_shader);

        // TODO: Materials need to be able to set up some pipeline
        // options, e.g. tessellation.

        let pipeline = state.gfx_pipes.get_or_create(&desc);
        cmds.bind_gfx_pipe(&pipeline);

        if i == 0 {
            // TODO: this could be done outside the loop
            cmds.bind_gfx_descs(0, view.uniform_desc());
            cmds.bind_gfx_descs(1, &instance_desc);
        }
        if let Some(desc) = material.desc.as_ref() {
            cmds.bind_gfx_descs(2, desc);
        }

        let fst_inst = i as u32;
        cmds.bind_vertex_buffers(&mesh.data());
        if let Some(ref index) = mesh.index() {
            let vert_count = index.count();
            cmds.bind_index_buffer(index.alloc.range(), index.ty);
            cmds.draw_indexed_offset(vert_count, 1, 0, 0, fst_inst);
        } else {
            let vert_count = mesh.vertex_count();
            cmds.draw_offset(vert_count, 1, 0, fst_inst);
        }
    }
}
