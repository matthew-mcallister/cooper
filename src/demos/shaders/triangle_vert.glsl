#version 450
#pragma shader_stage(vertex)

const vec2 VERTEX_POS[] = {
    vec2(-1, 1),
    vec2(1, 1),
    vec2(-1, -1),
    vec2(1, -1),
};
const vec3 VERTEX_COLOR[] = {
    vec3(1, 0, 0),
    vec3(0, 1, 0),
    vec3(0, 0, 1),
    vec3(1, 1, 0),
};

layout(location = 0) out vec4 vtx_color;

void main() {
    gl_Position = vec4(VERTEX_POS[gl_VertexIndex], 0, 1);
    vtx_color = vec4(VERTEX_COLOR[gl_VertexIndex], 1);
}
