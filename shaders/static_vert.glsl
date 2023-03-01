#version 450
#pragma shader_stage(vertex)

struct Perspective {
    mat4 proj;
    //mat4 proj_inv;
    float tan_fovx2;
    float tan_fovy2;
    float z_near;
    float z_far;
    float min_depth;
    float max_depth;
};

struct SceneView {
    Perspective perspective;
    mat4 view;
    //mat4 view_inv;
};

layout(set = 0, binding = 0) uniform blk_scene {
    SceneView g_scene_view;
};
layout(set = 0, binding = 1) readonly buffer blk_xforms {
    mat4 g_xforms[];
};

layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec3 in_normal;
layout(location = 4) in vec2 in_texcoord0;

layout(location = 0) out vec3 out_normal;
layout(location = 1) out vec2 out_texcoord0;

void main() {
    mat4 mv = g_xforms[gl_InstanceIndex];

    vec4 view_pos = mv * vec4(in_pos, 1);
    gl_Position = g_scene_view.perspective.proj * view_pos;

    out_normal = (mv * vec4(in_normal, 0.0)).xyz;
    out_texcoord0 = in_texcoord0;
}
