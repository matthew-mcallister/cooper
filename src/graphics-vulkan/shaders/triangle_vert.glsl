#version 450
#pragma shader_stage(vertex)

#include "common.inc"

const vec2 VERTEX_POS[] = {
    vec2(0, 1),
    vec2(1, 1),
    vec2(0, 0),
    vec2(1, 0),
};

layout(set = 1, binding = 0, row_major) readonly buffer SpriteBuf {
    Sprite sprites[];
};

layout(location = 0) out flat uvec2 vtx_textures;
layout(location = 1) out vec2 vtx_texcoord0;

void main() {
    Sprite sprite = sprites[gl_InstanceIndex];
    SpriteTransform xform = sprite.transform;

    vec2 pos0 = VERTEX_POS[gl_VertexIndex];
    vec2 pos = xform.mat * pos0 + xform.offs;

    gl_Position = vec4(pos, 0, 1);
    vtx_textures = sprite.textures;
    vtx_texcoord0 = pos0;
}
