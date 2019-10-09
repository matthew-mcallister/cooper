#version 450
#pragma shader_stage(vertex)

#include "example_inc.glsl"

layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec3 in_normal;

layout(location = 0) out vec4 out_world_pos;
layout(location = 1) out vec4 out_world_normal;

void main() {
    Instance inst = globals.instances[gl_InstanceIndex];

    vec4 model_pos = vec4(inst.scale.x * in_pos, 1);
    vec4 world_pos = inst.pos
        + inst.rgt * model_pos.x
        + inst.fwd * model_pos.y
        + inst.abv * model_pos.z;
    vec4 view_pos = globals.camera.view * world_pos;

    vec4 world_normal
        = inst.rgt * in_normal.x
        + inst.fwd * in_normal.y
        + inst.abv * in_normal.z;

    gl_Position = globals.camera.perspective * view_pos;
    out_world_pos = world_pos;
    out_world_normal = world_normal;
}
