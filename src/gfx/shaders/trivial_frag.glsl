#version 450
#pragma shader_stage(fragment)

layout(set = 1, binding = 0) uniform sampler2D image0;
layout(set = 1, binding = 1, rgba8) uniform readonly image2D image1;
layout(set = 1, binding = 2) uniform texture2D image2;

layout(location = 0) out vec4 out_color;

void main() {
    out_color = vec4(1.0, 0.0, 0.0, 1.0);
}
