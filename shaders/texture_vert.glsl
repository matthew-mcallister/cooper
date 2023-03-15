#version 450
#pragma shader_stage(vertex)

layout(set = 0, binding = 0) uniform blk_uniforms {
    mat4 u_mvp;
};

layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec2 in_st;

layout(location = 0) out vec2 out_st;

void main() {
    gl_Position = u_mvp * vec4(in_pos, 1);
    out_st = in_st;
}
