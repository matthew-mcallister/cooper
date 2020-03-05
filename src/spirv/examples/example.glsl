#version 450
#pragma shader_stage(vertex)

layout(set = 0, binding = 0) readonly buffer blk {
    mat4 view_proj;
};

layout(constant_id = 0) const uint MAX_BONES = 256;

layout(set = 1, binding = 0) uniform blk2 {
    mat4x3 bones[MAX_BONES];
};

layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in uvec4 in_joints;
layout(location = 3) in vec4 in_weights;

layout(location = 0) out vec2 out_uv;

void main() {
    mat4x3 xform = in_weights.x * bones[in_joints.x]
        + in_weights.y * bones[in_joints.y]
        + in_weights.z * bones[in_joints.z]
        + in_weights.w * bones[in_joints.w];
    vec4 pos = vec4(xform * vec4(in_pos, 1), 1);
    gl_Position = view_proj * pos;
    out_uv = in_uv;
}
