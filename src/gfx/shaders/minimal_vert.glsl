#version 450
#pragma shader_stage(vertex)

#include "common_inc.glsl"

layout(set = 0, binding = 0) uniform blk_scene {
    SceneView g_scene_view;
};
layout(set = 0, binding = 1) readonly buffer blk_xforms {
    mat4 g_xforms[];
};

layout(location = VERTEX_ATTR_POSITION) in vec3 in_pos;
layout(location = VERTEX_ATTR_NORMAL) in vec3 in_normal;

layout(location = 0) out vec3 out_normal;
layout(location = 1) out vec2 out_texcoord0;

void main() {
    mat4 mv = g_xforms[gl_InstanceIndex];

    vec4 view_pos = mv * vec4(in_pos, 1);
    gl_Position = g_scene_view.perspective.proj * view_pos;

    out_normal = (mv * vec4(in_normal, 0.0)).xyz;
    out_texcoord0 = vec2(0.0);
}
