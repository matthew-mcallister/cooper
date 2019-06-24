#version 450
#pragma shader_stage(vertex)

const vec2 VERTEX_POS[] = {
    vec2(0, 1),
    vec2(1, 1),
    vec2(0, 0),
    vec2(1, 0),
};
const vec3 VERTEX_COLOR[] = {
    vec3(1, 0, 0),
    vec3(0, 1, 0),
    vec3(0, 0, 1),
    vec3(1, 1, 0),
};

struct SpriteTransform {
    mat2 mat;
    vec2 offs;
};

struct Sprite {
    SpriteTransform transform;
    uvec2 textures;
};

layout(set = 0, binding = 0, row_major) readonly buffer SpriteBuf {
    Sprite sprites[];
};

layout(location = 0) out vec4 vtx_color;

void main() {
    Sprite sprite = sprites[gl_InstanceIndex];
    SpriteTransform xform = sprite.transform;

    vec2 pos0 = VERTEX_POS[gl_VertexIndex];
    vec2 pos = xform.mat * pos0 + xform.offs;

    gl_Position = vec4(pos, 0, 1);
    vtx_color = vec4(VERTEX_COLOR[gl_VertexIndex], 1);
}
