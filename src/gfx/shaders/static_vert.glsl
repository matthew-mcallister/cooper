#version 450
#pragma shader_stage(vertex)

#include "common_inc.glsl"

layout(set = 0, binding = 0) uniform blk_scene {
    SceneView g_scene_view;
};
layout(set = 0, binding = 1) readonly buffer blk_inst {
    Instance g_instances[];
};

layout(location = VERTEX_ATTR_POSITION) in vec3 in_pos;
layout(location = VERTEX_ATTR_NORMAL) in vec3 in_normal;

layout(location = 0) out vec3 out_normal;

void main() {
    vec4 xform[3] = g_instances[gl_InstanceIndex].xform;
    mat4 mv = transpose(mat4(xform[0], xform[1], xform[2], vec4(vec3(0), 1)));

    vec4 view_pos = mv * vec4(in_pos, 1);
    gl_Position = g_scene_view.perspective.proj * view_pos;

    out_normal = (mv * vec4(in_normal, 0.0)).xyz;
}
