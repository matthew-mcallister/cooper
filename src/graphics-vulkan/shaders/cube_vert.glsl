#version 450
#pragma shader_stage(vertex)

const uint VERTEX_IDX[] = {
    1, 0, 3, 2, 6, 0, 4, 1, 5, 3, 7, 6, 5, 4
};

const vec3 VERTEX_POS[] = {
    vec3(0, 0, 0),
    vec3(1, 0, 0),
    vec3(0, 1, 0),
    vec3(1, 1, 0),
    vec3(0, 0, 1),
    vec3(1, 0, 1),
    vec3(0, 1, 1),
    vec3(1, 1, 1)
};

const vec3 VERTEX_COL[] = {
    vec3(0, 0, 0),
    vec3(1, 0, 0),
    vec3(0, 1, 0),
    vec3(0, 0, 1),
    vec3(1, 1, 0),
    vec3(1, 0, 1),
    vec3(0, 1, 1),
    vec3(1, 1, 1)
};

layout(location = 0) out vec4 vtx_color;

void main() {
    uint idx = gl_VertexIndex;
    gl_Position = vec4(VERTEX_POS[idx], 1);
    vtx_color = vec4(VERTEX_COL[idx], 1);
}
