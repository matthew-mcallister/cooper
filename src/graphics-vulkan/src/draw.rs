use std::ptr;

use crate::*;

#[derive(Debug)]
crate struct SceneDraw<'a> {
    sys: &'a mut System,
    dt: Arc<vkl::DeviceTable>,
    pool: vk::CommandPool,
    cmds: vk::CommandBuffer,
}

impl SceneDraw {
    crate unsafe fn draw_scene(&mut self, objects: &ObjectStore) {
        let info = vk::CommandBufferBeginInfo {
            flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT_BIT,
            ..Default::default()
        };
        self.dt.begin_command_buffer(self.cmds, &info as _).check().unwrap();

        let render_pass = self.sys.render_passes.get("forward");
        let clear_values = [
            Default::default(),
            vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue {
                depth: 1.0,
                stencil: 0,
            } },
        ];
        let info = vk::RenderPassBeginInfo {
            render_pass: render_pass.obj,
            framebuffer: sys.cur_frame()[RenderPassName::Forward],
            render_area: sys.swapchain.extent,
            clear_value_count: clear_values.len() as _,
            p_clear_values: clear_values.as_ptr(),
            ..Default::default()
        };
        self.dt.cmd_begin_render_pass
            (self.cmds, &info as _, Default::default());

        // Depth pass
        for obj in objects.iter() {
            self.draw_depth(obj);
        }

        // Lighting pass
        self.dt.cmd_next_render_pass(self.cmds, Default::default());

        for obj in objects.iter() {
            self.draw_lighting(obj);
        }

        self.dt.cmd_end_render_pass(self.cmds);
        self.dt.end_command_buffer(self.cmds).check().unwrap();
    }

    fn draw_primitives(&mut self, geom: &Geometry) {
        self.dt.cmd_bind_index_buffer
            (self.cmds, geom.index_buffer, 0, geom.index_type);
        self.dt.cmd_bind_vertex_buffers
            (self.cmds, 0, 1, &geom.vert_buffer as _, &0 as _);
        self.cmd_draw_indexed
            (self.cmds, 3 * geom.primitive_count, 1, 0, 0, 0);
    }

    fn draw_depth(&mut self, obj: &Object) {
        if !obj.visible { return; }
        match &obj.data {
            ObjData::Static(obj) => self.draw_static_depth(obj),
        }
    }

    fn draw_static_depth(&mut self, obj: &StaticObj) {
        let geom = self.sys.resources.get(obj.geometry);

        let pipeline = self.sys.pipe_map.get(&PipelineDesc {
            geometry: PipelineGeometry::Static,
            output: PipelineOutput::Depth,
        });
        self.dt.cmd_bind_pipeline
            (self.cmds, vk::PipelineBindPoint::GRAPHICS, pipeline);

        let layout = self.sys.pipe_layouts.get("depth");
        self.dt.cmd_bind_descriptor_sets(
            self.cmds,
            vk::PipelineBindPoint::GRAPHICS,
            layout,
            0,
            1,
            &obj.desc,
            0,
            ptr::null(),
        );

        self.draw_primitives(geom);
    }

    fn draw_lighting(&mut self, obj: &SceneObj) {
        if !obj.visible { return; }
        match obj {
            ObjectData::Static(obj) => self.draw_static_lighting(obj),
        }
    }

    fn draw_static_lighting(&mut self, obj: &StaticObj) {
        let geom = self.sys.resources.get(obj.geometry);
        let mat = self.sys.resources.get(obj.material);

        let pipeline = self.sys.pipe_map.get(&PipelineDesc {
            geometry: PipelineGeometry::Rigid,
            output: PipelineOutput::Lighting {
                //frag_shader: mat.shader(),
                frag_shader: "pbr_frag",
                alpha_mode: None,
            },
        });
        self.dt.cmd_bind_pipeline
            (self.cmds, vk::PipelineBindPoint::GRAPHICS, pipeline);

        let layout = self.sys.pipe_layouts.get("pbr");
        let descs = [mat.desc, obj.desc];
        self.dt.cmd_bind_descriptor_sets(
            self.cmds,
            vk::PipelineBindPoint::GRAPHICS,
            layout,
            0,
            descs.len() as _,
            descs.as_ptr(),
            0,
            ptr::null(),
        );

        self.draw_primitives(geom);
    }
}
