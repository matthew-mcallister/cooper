#version 450
#pragma shader_stage(vertex)

layout(set = 0, binding = 0) uniform blk_uniforms {
    mat4 u_mvp;
    float u_time;
};

layout(location = 0) in vec3 in_pos;
layout(location = 0) out vec3 out_color;

void main() {
    out_color = in_pos * 0.5 + 0.5;
    gl_Position = u_mvp * vec4(in_pos, 1);
}
