#version 450
#pragma shader_stage(vertex)

layout(location = 0) out vec3 out_color;

const vec2 POSITIONS[] = {
    vec2(-0.5, 0.433),
    vec2(0.5, 0.433),
    vec2(0, -0.433)
};

const vec3 COLORS[] = {
    vec3(1, 0, 0),
    vec3(0, 1, 0),
    vec3(0, 0, 1)
};

void main() {
    int i = gl_VertexIndex % 3;
    vec2 pos = POSITIONS[i];
    gl_Position = vec4(pos, 0, 1);
    out_color = COLORS[i];
}
