#version 450
#pragma shader_stage(fragment)

layout(set = 1, binding = 0) uniform sampler2D image1;
layout(set = 1, binding = 1) uniform image2D image0;

layout(location = 0) out vec4 out_color;

void main() {
    out_color = vec4(1.0, 0.0, 0.0, 1.0);
}
