#version 450
#pragma shader_stage(vertex)

#include "example_inc.glsl"

layout(location = 0) in vec3 in_world_pos;
layout(location = 1) in vec3 in_world_normal;

layout(location = 0) out vec3 out_world_pos;
layout(location = 1) out vec3 out_world_normal;

void main() {
    Instance inst = g_instances[gl_InstanceIndex];

    vec3 pos = in_world_pos;
    vec3 normal = in_world_normal;

    vec3 model_pos = inst.scale.x * pos;
    vec4 world_pos = inst.pos
        + inst.rgt * model_pos.x
        + inst.fwd * model_pos.y
        + inst.abv * model_pos.z;
    vec4 view_pos = globals.camera.view * world_pos;

    vec4 world_normal
        = inst.rgt * normal.x
        + inst.fwd * normal.y
        + inst.abv * normal.z;

    gl_Position = globals.camera.perspective * view_pos;
    out_world_pos = world_pos.xyz;
    out_world_normal = world_normal.xyz;
}
