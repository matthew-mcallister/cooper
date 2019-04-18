#version 450
#pragma shader_stage(vertex)

layout(set = 2, binding = 0) uniform unif_object {
    mat4 mvp_matrix;
};

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_tex_coord_0;
layout(location = 3) in vec2 in_tangent;

void main() {
    gl_Position = mvp_matrix * vec4(in_position, 1.0);
}
