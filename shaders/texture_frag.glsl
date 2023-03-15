#version 450
#pragma shader_stage(fragment)

layout(set = 0, binding = 0) uniform blk_uniforms {
    mat4 u_mvp;
};
layout(set = 0, binding = 1) uniform sampler2D u_texture;

layout(location = 0) in vec2 in_st;

layout(location = 0) out vec4 out_color;

void main() {
    out_color = texture(u_texture, in_st);
}
