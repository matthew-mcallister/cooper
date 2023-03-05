#version 450
#pragma shader_stage(fragment)

layout(set = 0, binding = 0) uniform blk_uniforms {
    mat4 u_mvp;
    float u_time;
};

layout(location = 0) in vec3 in_color;
layout(location = 0) out vec4 out_color;

void main() {
    out_color = vec4(in_color, 1);
}
