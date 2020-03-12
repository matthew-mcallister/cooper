#version 450
#pragma shader_stage(vertex)

layout(location = 0) in vec3 in_pos;

layout(location = 0) out vec3 out_normal;

void main() {
    out_normal = vec3(0.0, 0.0, -1.0);
    gl_Position = vec4(in_pos, 1.0);
}
