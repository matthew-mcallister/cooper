#version 450
#pragma shader_stage(vertex)

layout(set = 0, binding = 0) uniform blk1 {
    uint a_var;
};

layout(set = 0, binding = 1) readonly buffer blk2 {
    float another_var;
};

const vec3 POS[] = vec3[](
    vec3(-1.0,  1.0, 0.0),
    vec3( 1.0,  1.0, 0.0),
    vec3(-1.0, -1.0, 0.0));

void main() {
    gl_Position = vec4(POS[gl_VertexIndex % 3], 1.0);
}
