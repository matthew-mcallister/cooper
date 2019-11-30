#version 450
#pragma shader_stage(vertex)

const vec3 VERTEX_POS[] = {
    vec3(-1, -1, 0),
    vec3(-1, 1, 0),
    vec3(1, 1, 0),
};

const vec3 VERTEX_COLOR[] = {
    vec3(1, 0, 0),
    vec3(0, 1, 0),
    vec3(0, 0, 1),
};

layout(location = 0) out vec4 vtx_color;

void main() {
    uint vtx = gl_VertexIndex;
    gl_Position = vec4(VERTEX_POS[vtx], 1);
    vtx_color = vec4(VERTEX_COLOR[vtx], 1);
}
