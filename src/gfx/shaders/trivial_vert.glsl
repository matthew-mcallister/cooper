#version 450
#pragma shader_stage(vertex)

#include "global_inc.glsl"
#include "trivial_inc.glsl"

// TODO driver-level index buffer maybe
const uint INDEX[] = uint[](
    // Z = 0
    0, 2, 1,
    1, 2, 3,
    // Z = 1
    4, 5, 6,
    6, 5, 7,
    // Y = 0
    0, 1, 4,
    4, 1, 5,
    // Y = 1
    2, 6, 3,
    3, 6, 7,
    // X = 0
    2, 0, 6,
    6, 0, 4,
    // X = 1
    1, 3, 5,
    5, 3, 7);

const vec3 POS[] = vec3[](
    vec3(0.0, 0.0, 0.0),
    vec3(1.0, 0.0, 0.0),
    vec3(0.0, 1.0, 0.0),
    vec3(1.0, 1.0, 0.0),

    vec3(0.0, 0.0, 1.0),
    vec3(1.0, 0.0, 1.0),
    vec3(0.0, 1.0, 1.0),
    vec3(1.0, 1.0, 1.0));

mat3 cols2mat3(vec4 cols[3]) {
    return mat3(cols[0].xyz, cols[1].xyz, cols[2].xyz);
}

layout(location = 0) out vec3 out_color;

void main() {
    TrivialInstance inst = get_inst();

    uint idx = INDEX[gl_VertexIndex];
    mat3 rot = cols2mat3(inst.rot_cols);

    vec3 pos = POS[idx];
    vec3 model_pos = (2.0 * pos - 1.0) * inst.radius.xyz;
    vec3 world_pos = rot * model_pos + inst.offs.xyz;
    vec4 view_pos = g_globals.camera.view * vec4(world_pos, 1.0);

    gl_Position = g_globals.camera.perspective * view_pos;
    out_color = inst.colors[idx].rgb;
}
