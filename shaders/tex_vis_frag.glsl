#version 450
#pragma shader_stage(fragment)

#include "common_inc.glsl"

layout(constant_id = CONST_ID_TEXTURE_VIS_SLOT)
const uint TEXTURE_VIS_SLOT = IMAGE_SLOT_ALBEDO;

layout(set = 1, binding = 0) uniform sampler2D u_images[IMAGE_SLOT_MAX];

layout(location = 0) in vec3 in_normal;
layout(location = 1) in vec2 in_texcoord0;

layout(location = 0) out vec4 out_color;

void main() {
    out_color = texture(u_images[TEXTURE_VIS_SLOT], in_texcoord0);
}
