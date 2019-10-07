#version 450
#pragma shader_stage(fragment)

layout(location = 0) in vec4 vtx_color;
layout(location = 0) out vec4 frag_color;

void main() {
    frag_color = vtx_color;
}
