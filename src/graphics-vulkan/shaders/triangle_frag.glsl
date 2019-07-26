#version 450
#pragma shader_stage(fragment)

#extension GL_EXT_nonuniform_qualifier : require

#include "common.inc"

layout(set = 0, binding = 0) uniform sampler2D textures[];

layout(location = 0) in flat uvec2 vtx_textures;
layout(location = 1) in vec2 vtx_texcoord0;
layout(location = 0) out vec4 frag_color;

void main() {
    frag_color =
        texture(textures[nonuniformEXT(vtx_textures.x)], vtx_texcoord0);
}
