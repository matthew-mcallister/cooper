#version 450
#pragma shader_stage(vertex)

#include "common_inc.glsl"

struct DebugInstance {
    mat4 mv;
    // TODO:
    //mat4 mvp;
};

layout(set = 0, binding = 0) uniform blk_scene {
    SceneView g_scene_view;
};
layout(set = 0, binding = 1) readonly buffer blk_inst {
    DebugInstance g_instances[];
};

layout(location = 0) in vec3 in_pos;

layout(location = 0) out vec3 out_normal;

void main() {
    DebugInstance inst = g_instances[gl_InstanceIndex];
    vec4 view_pos = inst.mv * vec4(in_pos, 1);
    gl_Position = g_scene_view.perspective.proj * view_pos;

    out_normal = vec3(0.0, 0.0, -1.0);
}
